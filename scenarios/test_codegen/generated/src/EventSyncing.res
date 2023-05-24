exception QueryTimout(string)

let convertLogs = (
  logsPromise: Promise.t<array<Ethers.log>>,
  ~provider,
  ~addressInterfaceMapping,
  ~fromBlockForLogging,
  ~toBlockForLogging,
  ~chainId,
) => {
  let blockRequestMapping: Js.Dict.t<
    Promise.t<Js.Nullable.t<Ethers.JsonRpcProvider.block>>,
  > = Js.Dict.empty()

  //Many times logs will be from the same block so there is no need to make multiple get block requests in that case
  let getMemoisedBlockPromise = blockNumber => {
    let blockRequestCached = blockRequestMapping->Js.Dict.get(blockNumber->Belt.Int.toString)

    let blockRequest = switch blockRequestCached {
    | Some(req) => req
    | None =>
      let newRequest = provider->Ethers.JsonRpcProvider.getBlock(blockNumber)
      blockRequestMapping->Js.Dict.set(blockNumber->Belt.Int.toString, newRequest)

      newRequest
    }
    blockRequest->Promise.then(block =>
      switch block->Js.Nullable.toOption {
      | Some(block) => Promise.resolve(block)
      | None =>
        Promise.reject(
          Js.Exn.raiseError(`getBLock(${blockNumber->Belt.Int.toString}) returned null`),
        )
      }
    ) // dangerous to not catch here but need to catch this promise later where it is used and handle it there
  }

  let task = async () => {
    let logs = await logsPromise

    Js.log2("Handling number of logs: ", logs->Array.length)

    let events =
      await logs
      ->Belt.Array.map(log => {
        let blockPromise = log.blockNumber->getMemoisedBlockPromise

        //get a specific interface type
        //interface type parses the log
        let optInterface = addressInterfaceMapping->Js.Dict.get(log.address->Obj.magic)

        switch optInterface {
        | None => None
        | Some(interface) => {
            let logDescription = interface->Ethers.Interface.parseLog(~log)

            switch Converters.eventStringToEvent(
              logDescription.name,
              Converters.getContractNameFromAddress(log.address, chainId),
            ) {
            | GravatarContract_NewGravatarEvent =>
              let convertedEvent =
                logDescription
                ->Converters.Gravatar.convertNewGravatarLogDescription
                ->Converters.Gravatar.convertNewGravatarLog(~log, ~blockPromise)

              Some(convertedEvent)
            | GravatarContract_UpdatedGravatarEvent =>
              let convertedEvent =
                logDescription
                ->Converters.Gravatar.convertUpdatedGravatarLogDescription
                ->Converters.Gravatar.convertUpdatedGravatarLog(~log, ~blockPromise)

              Some(convertedEvent)
            | NftFactoryContract_SimpleNftCreatedEvent =>
              let convertedEvent =
                logDescription
                ->Converters.NftFactory.convertSimpleNftCreatedLogDescription
                ->Converters.NftFactory.convertSimpleNftCreatedLog(~log, ~blockPromise)

              Some(convertedEvent)
            | SimpleNftContract_TransferEvent =>
              let convertedEvent =
                logDescription
                ->Converters.SimpleNft.convertTransferLogDescription
                ->Converters.SimpleNft.convertTransferLog(~log, ~blockPromise)

              Some(convertedEvent)
            }
          }
        }
      })
      ->Belt.Array.keepMap(opt => opt)
      ->Promise.all

    events
  }

  Time.retryOnCatchAfterDelay(
    ~retryDelayMilliseconds=5000,
    ~retryMessage=`Failed to handle event logs from block ${fromBlockForLogging->Belt.Int.toString} to block ${toBlockForLogging->Belt.Int.toString}`,
    ~task,
  )
}

let makeCombinedEventFilterQuery = (~provider, ~eventFilters, ~fromBlock, ~toBlock) => {
  open Ethers.BlockTag

  let combinedFilter =
    eventFilters->Ethers.CombinedFilter.combineEventFilters(
      ~fromBlock=BlockNumber(fromBlock)->blockTagFromVariant,
      ~toBlock=BlockNumber(toBlock)->blockTagFromVariant,
    )

  Js.log3("Intiating Combined Query Filter fromBlock toBlock: ", fromBlock, toBlock)

  let task = () =>
    provider
    ->Ethers.JsonRpcProvider.getLogs(
      ~filter={combinedFilter->Ethers.CombinedFilter.combinedFilterToFilter},
    )
    ->Promise.thenResolve(res => {
      Js.log3("Successful Combined Query Filter fromBlock toBlock: ", fromBlock, toBlock)
      res
    })

  Time.retryOnCatchAfterDelay(
    ~retryDelayMilliseconds=5000,
    ~retryMessage=`Failed combined query filter from block ${fromBlock->Belt.Int.toString} to block ${toBlock->Belt.Int.toString}`,
    ~task,
  )
}

let queryEventsWithCombinedFilterAndExecuteHandlers = async (
  ~addressInterfaceMapping,
  ~eventFilters,
  ~fromBlock,
  ~toBlock,
  ~provider,
  ~chainId,
) => {
  let combinedFilter = makeCombinedEventFilterQuery(~provider, ~eventFilters, ~fromBlock, ~toBlock)
  let events =
    await combinedFilter->convertLogs(
      ~provider,
      ~addressInterfaceMapping,
      ~fromBlockForLogging=fromBlock,
      ~toBlockForLogging=toBlock,
      ~chainId,
    )

  events->EventProcessing.processEventBatch(~chainId)
}

let getAllEventFilters = (
  ~addressInterfaceMapping,
  ~chainConfig: Config.chainConfig,
  ~provider,
) => {
  let eventFilters = []

  chainConfig.contracts->Belt.Array.forEach(contract => {
    let contractEthers = Ethers.Contract.make(
      ~address=contract.address,
      ~abi=contract.abi,
      ~provider,
    )
    addressInterfaceMapping->Js.Dict.set(
      contract.address->Ethers.ethAddressToString,
      contractEthers->Ethers.Contract.getInterface,
    )

    contract.events->Belt.Array.forEach(eventName => {
      let eventFilter =
        contractEthers->Ethers.Contract.getEventFilter(
          ~eventName=Types.eventNameToString(eventName),
        )
      let _ = eventFilters->Js.Array2.push(eventFilter)
    })
  })
  eventFilters
}

type blocksProcessed = {
  from: int,
  to: int,
}

let processAllEventsFromBlockNumber = async (
  ~fromBlock: int,
  ~blockInterval as maxBlockInterval,
  ~chainConfig: Config.chainConfig,
  ~provider,
) => {
  let addressInterfaceMapping: Js.Dict.t<Ethers.Interface.t> = Js.Dict.empty()

  let eventFilters = getAllEventFilters(~addressInterfaceMapping, ~chainConfig, ~provider)

  let fromBlockRef = ref(fromBlock)
  let currentBlock: ref<option<int>> = ref(None)
  let shouldContinueProcess = () =>
    currentBlock.contents->Belt.Option.mapWithDefault(true, blockNum =>
      fromBlockRef.contents < blockNum
    )

  while shouldContinueProcess() {
    let rec executeQuery = (~blockInterval) => {
      //If the query hangs for longer than 20 seconds, reject this promise to reduce the block interval
      let queryTimoutPromise =
        Time.resolvePromiseAfterDelay(~delayMilliseconds=20000)->Promise.then(() =>
          Promise.reject(QueryTimout("Query took longer than 20 seconds"))
        )

      let queryPromise =
        queryEventsWithCombinedFilterAndExecuteHandlers(
          ~addressInterfaceMapping,
          ~eventFilters,
          ~fromBlock=fromBlockRef.contents,
          ~toBlock=fromBlockRef.contents + blockInterval - 1,
          ~provider,
          ~chainId=chainConfig.chainId,
        )->Promise.thenResolve(_ => blockInterval)

      [queryTimoutPromise, queryPromise]
      ->Promise.race
      ->Promise.catch(err => {
        Js.log2("Error getting events, waiting 5 seconds before retrying", err)

        Time.resolvePromiseAfterDelay(~delayMilliseconds=5000)->Promise.then(_ => {
          let nextBlockIntervalTry = (blockInterval->Belt.Int.toFloat *. 0.8)->Belt.Int.fromFloat
          Js.log3("Retrying query fromBlock and toBlock:", fromBlock, nextBlockIntervalTry)
          executeQuery(~blockInterval={nextBlockIntervalTry})
        })
      })
    }

    let executedBlockInterval = await executeQuery(~blockInterval=maxBlockInterval)

    fromBlockRef := fromBlockRef.contents + executedBlockInterval
    let currentBlockFromRPC =
      await provider
      ->Ethers.JsonRpcProvider.getBlockNumber
      ->Promise.catch(_err => {
        Js.log("Error getting current block number")
        currentBlock.contents->Belt.Option.getWithDefault(0)->Promise.resolve
      })
    currentBlock := Some(currentBlockFromRPC)
    Js.log(
      `Finished processAllEventsFromBlockNumber ${fromBlockRef.contents->Belt.Int.toString} out of ${currentBlockFromRPC->Belt.Int.toString}`,
    )
  }
  {from: fromBlock, to: fromBlockRef.contents - 1}
}

let processAllEvents = async (chainConfig: Config.chainConfig) => {
  let latestProcessedBlock = await DbFunctions.RawEvents.getLatestProcessedBlockNumber(
    ~chainId=chainConfig.chainId,
  )

  let startBlock =
    latestProcessedBlock->Belt.Option.mapWithDefault(
      chainConfig.startBlock,
      latestProcessedBlock => {latestProcessedBlock + 1},
    )

  await processAllEventsFromBlockNumber(
    ~fromBlock=startBlock,
    ~chainConfig,
    ~blockInterval=2000,
    ~provider=chainConfig.provider,
  )
}

let startSyncingAllEvents = () => {
  Config.config
  ->Js.Dict.values
  ->Belt.Array.map(chainConfig => {
    chainConfig->processAllEvents
  })
  ->Promise.all
  ->Promise.thenResolve(_ => ())
}
