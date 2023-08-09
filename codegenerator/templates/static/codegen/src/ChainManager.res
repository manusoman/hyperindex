type t = {
  chainFetchers: Js.Dict.t<ChainFetcher.t>,
  //The priority queue should only house the latest event from each chain
  //And potentially extra events that are pushed on by newly registered dynamic
  //contracts which missed being fetched by they chainFetcher
  arbitraryEventPriorityQueue: SDSL.PriorityQueue.t<EventFetching.eventBatchQueueItem>,
}

let getComparitorFromItem = (queueItem: EventFetching.eventBatchQueueItem) => {
  let {timestamp, chainId, blockNumber, logIndex} = queueItem
  EventUtils.getEventComparator({timestamp, chainId, blockNumber, logIndex})
}

let priorityQueueComparitor = (
  a: EventFetching.eventBatchQueueItem,
  b: EventFetching.eventBatchQueueItem,
) => {
  if a->getComparitorFromItem < b->getComparitorFromItem {
    -1
  } else {
    1
  }
}

let chainFetcherPeekComparitorEarliestEvent = (
  a: ChainFetcher.eventQueuePeek,
  b: ChainFetcher.eventQueuePeek,
): bool => {
  switch (a, b) {
  | (Item(itemA), Item(itemB)) => itemA->getComparitorFromItem < itemB->getComparitorFromItem
  | (Item(itemA), NoItem(latestFetchedBlockTimestampB, chainId)) =>
    (itemA.timestamp, itemA.chainId) < (latestFetchedBlockTimestampB, chainId)
  | (NoItem(latestFetchedBlockTimestampA, chainId), Item(itemB)) =>
    (latestFetchedBlockTimestampA, chainId) < (itemB.timestamp, itemB.chainId)
  | (
      NoItem(latestFetchedBlockTimestampA, chainIdA),
      NoItem(latestFetchedBlockTimestampB, chainIdB),
    ) =>
    (latestFetchedBlockTimestampA, chainIdA) < (latestFetchedBlockTimestampB, chainIdB)
  }
}

type nextEventErr = NoItemsInArray

let determineNextEvent = (chainFetchersPeeks: array<ChainFetcher.eventQueuePeek>): result<
  ChainFetcher.eventQueuePeek,
  nextEventErr,
> => {
  let nextItem = chainFetchersPeeks->Belt.Array.reduce(None, (accum, valB) => {
    switch accum {
    | None => Some(valB)
    | Some(valA) =>
      if chainFetcherPeekComparitorEarliestEvent(valA, valB) {
        Some(valA)
      } else {
        Some(valB)
      }
    }
  })

  switch nextItem {
  | None => Error(NoItemsInArray)
  | Some(item) => Ok(item)
  }
}

let make = (~configs: Config.chainConfigs, ~maxQueueSize): t => {
  let chainFetchers =
    configs
    ->Js.Dict.entries
    ->Belt.Array.map(((key, chainConfig)) => {
      (key, ChainFetcher.make(~chainConfig, ~maxQueueSize))
    })
    ->Js.Dict.fromArray
  {
    chainFetchers,
    arbitraryEventPriorityQueue: SDSL.PriorityQueue.makeAdvanced([], priorityQueueComparitor),
  }
}

let startFetchers = (self: t) => {
  self.chainFetchers
  ->Js.Dict.values
  ->Belt.Array.forEach(fetcher => {
    //Start the fetchers
    fetcher->ChainFetcher.startFetchingEvents->ignore
  })
}

exception UndefinedChain(Types.chainId)

let getChainFetcher = (self: t, ~chainId: int): ChainFetcher.t => {
  switch self.chainFetchers->Js.Dict.get(chainId->Belt.Int.toString) {
  | None =>
    Logging.error(`Undefined chain ${chainId->Belt.Int.toString} in chain manager`)
    UndefinedChain(chainId)->raise
  | Some(fetcher) => fetcher
  }
}

//Synchronus operation that returns an optional value and will not wait
//for a value to be on the queue
//TODO: investigate can this function + Async version below be combined to share
//logic
let popBatchItem = (self: t): option<EventFetching.eventBatchQueueItem> => {
  //Peek all next fetched event queue items on all chain fetchers
  let peekChainFetcherFrontItems =
    self.chainFetchers
    ->Js.Dict.values
    ->Belt.Array.map(fetcher => fetcher->ChainFetcher.peekFrontItemOfQueue)

  //Compare the peeked items and determine the next item
  let nextItemFromBuffer = peekChainFetcherFrontItems->determineNextEvent->Belt.Result.getExn

  //Callback for handling popping of chain fetcher events
  let popNextItem = () => {
    switch nextItemFromBuffer {
    | ChainFetcher.NoItem(_, _) => None
    | ChainFetcher.Item(batchItem) =>
      //If there is an item pop it off of the chain fetcher queue and return
      let fetcher = self->getChainFetcher(~chainId=batchItem.chainId)
      fetcher->ChainFetcher.popQueueItem
    }
  }

  //Peek arbitraty events queue
  let peekedArbTopItem = self.arbitraryEventPriorityQueue->SDSL.PriorityQueue.top

  switch peekedArbTopItem {
  //If there is item on the arbitray events queue, pop the relevant item from
  //the chain fetcher queue
  | None => popNextItem()
  | Some(peekedArbItem) =>
    //If there is an item on the arbitrary events queue, compare it to the next
    //item from the chain fetchers
    let arbItemIsEarlier = chainFetcherPeekComparitorEarliestEvent(
      ChainFetcher.Item(peekedArbItem),
      nextItemFromBuffer,
    )

    //If the arbitrary item is earlier, return that
    if arbItemIsEarlier {
      Some(
        //safely pop the item since we have already checked there's one at the front
        self.arbitraryEventPriorityQueue
        ->SDSL.PriorityQueue.pop
        ->Belt.Option.getUnsafe,
      )
    } else {
      //Else pop the next item from chain fetchers
      popNextItem()
    }
  }
}

//Async pop function that will wait for an item to be available before returning
//TODO: investigate combining logic with the above synchronus version of this function
let rec popAndAwaitBatchItem: t => promise<EventFetching.eventBatchQueueItem> = async (
  self: t,
): EventFetching.eventBatchQueueItem => {
  //Peek all next fetched event queue items on all chain fetchers
  let peekChainFetcherFrontItems =
    self.chainFetchers
    ->Js.Dict.values
    ->Belt.Array.map(fetcher => fetcher->ChainFetcher.peekFrontItemOfQueue)

  //Compare the peeked items and determine the next item
  let nextItemFromBuffer = peekChainFetcherFrontItems->determineNextEvent->Belt.Result.getExn

  //Callback for handling popping of chain fetcher events
  let popNextItemAndAwait = async () => {
    switch nextItemFromBuffer {
    | ChainFetcher.NoItem(_, chainId) =>
      //If higest priority is a "NoItem", it means we need to wait for
      //that chain fetcher to fetch blocks of a higher timestamp
      let fetcher = self->getChainFetcher(~chainId)
      //Add a callback and wait for a new block range to finish being queried
      await fetcher->ChainFetcher.addNewRangeQueriedCallback
      //Once there is confirmation from the chain fetcher that a new range has been
      //queried retry the popAwait batch function
      await self->popAndAwaitBatchItem
    | ChainFetcher.Item(batchItem) =>
      //If there is an item pop it off of the chain fetcher queue and return
      let fetcher = self->getChainFetcher(~chainId=batchItem.chainId)
      await fetcher->ChainFetcher.popAndAwaitQueueItem
    }
  }

  //Peek arbitraty events queue
  let peekedArbTopItem = self.arbitraryEventPriorityQueue->SDSL.PriorityQueue.top

  switch peekedArbTopItem {
  //If there is item on the arbitray events queue, pop the relevant item from
  //the chain fetcher queue
  | None => await popNextItemAndAwait()
  | Some(peekedArbItem) =>
    //If there is an item on the arbitrary events queue, compare it to the next
    //item from the chain fetchers
    let arbItemIsEarlier = chainFetcherPeekComparitorEarliestEvent(
      ChainFetcher.Item(peekedArbItem),
      nextItemFromBuffer,
    )

    //If the arbitrary item is earlier, return that
    if arbItemIsEarlier {
      //safely pop the item since we have already checked there's one at the front
      self.arbitraryEventPriorityQueue->SDSL.PriorityQueue.pop->Belt.Option.getUnsafe
    } else {
      //Else pop the next item from chain fetchers
      await popNextItemAndAwait()
    }
  }
}

let createBatch = async (self: t, ~minBatchSize: int, ~maxBatchSize: int): array<
  EventFetching.eventBatchQueueItem,
> => {
  let batch = []
  while batch->Belt.Array.length < minBatchSize {
    let item = await self->popAndAwaitBatchItem
    batch->Js.Array2.push(item)->ignore
  }

  let moreItemsToPop = ref(true)
  while moreItemsToPop.contents && batch->Belt.Array.length < maxBatchSize {
    let optItem = self->popBatchItem
    switch optItem {
    | None => moreItemsToPop := false
    | Some(item) => batch->Js.Array2.push(item)->ignore
    }
  }

  batch
}

let addItemToArbitraryEvents = (self: t, item: EventFetching.eventBatchQueueItem) => {
  self.arbitraryEventPriorityQueue->SDSL.PriorityQueue.push(item)->ignore
}
