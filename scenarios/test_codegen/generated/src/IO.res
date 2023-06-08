module InMemoryStore = {
  let entityCurrentCrud = (currentCrud: option<Types.crud>, nextCrud: Types.crud) => {
    switch (currentCrud, nextCrud) {
    | (Some(Create), Create) => Types.Create
    | (Some(Read), Create)
    | (Some(Update), Create)
    | (Some(Delete), Create) =>
      // dont know if this is an update or create
      Update
    | (Some(Create), Read) => Create
    | (Some(Read), Read) => Read
    | (Some(Update), Read) => Update
    | (Some(Delete), Read) => Delete
    | (Some(Create), Update) => Create
    | (Some(Read), Update) => Update
    | (Some(Update), Update) => Update
    | (Some(Delete), Update) => Update
    | (Some(Create), Delete) => Delete // interesting to note to line 23
    | (Some(Read), Delete) => Delete
    | (Some(Update), Delete) => Delete
    | (Some(Delete), Delete) => Delete
    | (None, _) => nextCrud
    }
  }

  module RawEvents = {
    let rawEventsDict: ref<Js.Dict.t<Types.inMemoryStoreRow<Types.rawEventsEntity>>> = ref(
      Js.Dict.empty(),
    )

    let getRawEvents = (~id: string) => {
      let row = Js.Dict.get(rawEventsDict.contents, id)
      row->Belt.Option.map(row => row.entity)
    }

    let setRawEvents = (~entity: Types.rawEventsEntity, ~crud: Types.crud) => {
      let key = EventUtils.getEventIdKeyString(~chainId=entity.chainId, ~eventId=entity.eventId)
      let rawEventCurrentCrud =
        rawEventsDict.contents
        ->Js.Dict.get(key)
        ->Belt.Option.map(row => {
          row.crud
        })

      rawEventsDict.contents->Js.Dict.set(
        key,
        {
          eventData: {chainId: entity.chainId, eventId: entity.eventId},
          entity,
          crud: entityCurrentCrud(rawEventCurrentCrud, crud),
        },
      )
    }
  }
  module DynamicContractRegistry = {
    let dynamicContractRegistryDict: ref<
      Js.Dict.t<Types.inMemoryStoreRow<Types.dynamicContractRegistryEntity>>,
    > = ref(Js.Dict.empty())

    let getDynamicContractRegistry = (~id: string) => {
      let row = Js.Dict.get(dynamicContractRegistryDict.contents, id)
      row->Belt.Option.map(row => row.entity)
    }

    let setDynamicContractRegistry = (
      ~entity: Types.dynamicContractRegistryEntity,
      ~crud: Types.crud,
    ) => {
      let key = EventUtils.getContractAddressKeyString(
        ~chainId=entity.chainId,
        ~contractAddress=entity.contractAddress,
      )
      let dynamicContractRegistryCurrentCrud =
        dynamicContractRegistryDict.contents
        ->Js.Dict.get(key)
        ->Belt.Option.map(row => {
          row.crud
        })

      dynamicContractRegistryDict.contents->Js.Dict.set(
        key,
        {
          eventData: {chainId: entity.chainId, eventId: entity.eventId->Ethers.BigInt.toString},
          entity,
          crud: entityCurrentCrud(dynamicContractRegistryCurrentCrud, crud),
        },
      )
    }
  }

  module User = {
    let userDict: ref<Js.Dict.t<Types.inMemoryStoreRow<Types.userEntity>>> = ref(Js.Dict.empty())

    let getUser = (~id: string) => {
      let row = Js.Dict.get(userDict.contents, id)
      row->Belt.Option.map(row => row.entity)
    }

    let setUser = (~entity: Types.userEntity, ~crud: Types.crud, ~eventData: Types.eventData) => {
      let userCurrentCrud = Js.Dict.get(userDict.contents, entity.id)->Belt.Option.map(row => {
        row.crud
      })

      userDict.contents->Js.Dict.set(
        entity.id,
        {eventData, entity, crud: entityCurrentCrud(userCurrentCrud, crud)},
      )
    }
  }

  module Gravatar = {
    let gravatarDict: ref<Js.Dict.t<Types.inMemoryStoreRow<Types.gravatarEntity>>> = ref(
      Js.Dict.empty(),
    )

    let getGravatar = (~id: string) => {
      let row = Js.Dict.get(gravatarDict.contents, id)
      row->Belt.Option.map(row => row.entity)
    }

    let setGravatar = (
      ~entity: Types.gravatarEntity,
      ~crud: Types.crud,
      ~eventData: Types.eventData,
    ) => {
      let gravatarCurrentCrud = Js.Dict.get(
        gravatarDict.contents,
        entity.id,
      )->Belt.Option.map(row => {
        row.crud
      })

      gravatarDict.contents->Js.Dict.set(
        entity.id,
        {eventData, entity, crud: entityCurrentCrud(gravatarCurrentCrud, crud)},
      )
    }
  }

  module Nftcollection = {
    let nftcollectionDict: ref<Js.Dict.t<Types.inMemoryStoreRow<Types.nftcollectionEntity>>> = ref(
      Js.Dict.empty(),
    )

    let getNftcollection = (~id: string) => {
      let row = Js.Dict.get(nftcollectionDict.contents, id)
      row->Belt.Option.map(row => row.entity)
    }

    let setNftcollection = (
      ~entity: Types.nftcollectionEntity,
      ~crud: Types.crud,
      ~eventData: Types.eventData,
    ) => {
      let nftcollectionCurrentCrud = Js.Dict.get(
        nftcollectionDict.contents,
        entity.id,
      )->Belt.Option.map(row => {
        row.crud
      })

      nftcollectionDict.contents->Js.Dict.set(
        entity.id,
        {eventData, entity, crud: entityCurrentCrud(nftcollectionCurrentCrud, crud)},
      )
    }
  }

  module Token = {
    let tokenDict: ref<Js.Dict.t<Types.inMemoryStoreRow<Types.tokenEntity>>> = ref(Js.Dict.empty())

    let getToken = (~id: string) => {
      let row = Js.Dict.get(tokenDict.contents, id)
      row->Belt.Option.map(row => row.entity)
    }

    let setToken = (~entity: Types.tokenEntity, ~crud: Types.crud, ~eventData: Types.eventData) => {
      let tokenCurrentCrud = Js.Dict.get(tokenDict.contents, entity.id)->Belt.Option.map(row => {
        row.crud
      })

      tokenDict.contents->Js.Dict.set(
        entity.id,
        {eventData, entity, crud: entityCurrentCrud(tokenCurrentCrud, crud)},
      )
    }
  }
  let resetStore = () => {
    User.userDict := Js.Dict.empty()
    Gravatar.gravatarDict := Js.Dict.empty()
    Nftcollection.nftcollectionDict := Js.Dict.empty()
    Token.tokenDict := Js.Dict.empty()
  }
}

type uniqueEntityReadIds = Js.Dict.t<Types.id>
type allEntityReads = Js.Dict.t<uniqueEntityReadIds>

let loadEntities = async (sql, entityBatch: array<Types.entityRead>) => {
  let uniqueUserDict = Js.Dict.empty()
  let uniqueGravatarDict = Js.Dict.empty()
  let uniqueNftcollectionDict = Js.Dict.empty()
  let uniqueTokenDict = Js.Dict.empty()

  // TODO: don't create an array if the entity doesn't have any entity relation fields.
  let populateUserLoadAsEntityFunctions: array<unit => unit> = []
  let populateGravatarLoadAsEntityFunctions: array<unit => unit> = []
  let populateNftcollectionLoadAsEntityFunctions: array<unit => unit> = []
  let populateTokenLoadAsEntityFunctions: array<unit => unit> = []

  let uniqueUserAsEntityFieldArray: array<string> = []
  let uniqueGravatarAsEntityFieldArray: array<string> = []
  let uniqueNftcollectionAsEntityFieldArray: array<string> = []
  let uniqueTokenAsEntityFieldArray: array<string> = []

  entityBatch->Belt.Array.forEach(readEntity => {
    switch readEntity {
    | UserRead(entityId, userLoad) =>
      let _ = Js.Dict.set(uniqueUserDict, entityId, entityId)
      switch userLoad.loadGravatar {
      | Some(
          _ /* TODO: read this and recursively add loaders. See: https://github.com/Float-Capital/indexer/issues/293 */,
        ) =>
        let _ = populateUserLoadAsEntityFunctions->Js.Array2.push(() => {
          let _ = InMemoryStore.User.getUser(~id=entityId)->Belt.Option.map(
            userEntity => {
              userEntity.gravatar->Belt.Option.map(
                gravatarId =>
                  switch uniqueGravatarDict->Js.Dict.get(gravatarId) {
                  | Some(_) => () // Already loaded
                  | None =>
                    let _ = uniqueGravatarAsEntityFieldArray->Js.Array2.push(gravatarId)
                    Js.Dict.set(uniqueGravatarDict, gravatarId, gravatarId)
                  },
              )
            },
          )
        })
      | None => ()
      }
      switch userLoad.loadTokens {
      | Some(
          _ /* TODO: read this and recursively add loaders. See: https://github.com/Float-Capital/indexer/issues/293 */,
        ) =>
        let _ = populateUserLoadAsEntityFunctions->Js.Array2.push(() => {
          let _ = InMemoryStore.User.getUser(~id=entityId)->Belt.Option.map(
            userEntity => {
              let _ = userEntity.tokens->Belt.Array.map(
                tokensId =>
                  switch uniqueTokenDict->Js.Dict.get(tokensId) {
                  | Some(_) => // Already loaded
                    ()
                  | None =>
                    let _ = uniqueTokenAsEntityFieldArray->Js.Array2.push(tokensId)
                    Js.Dict.set(uniqueTokenDict, tokensId, tokensId)
                  },
              )
            },
          )
        })
      | None => ()
      }
    | GravatarRead(entityId, gravatarLoad) =>
      let _ = Js.Dict.set(uniqueGravatarDict, entityId, entityId)
      switch gravatarLoad.loadOwner {
      | Some(
          _ /* TODO: read this and recursively add loaders. See: https://github.com/Float-Capital/indexer/issues/293 */,
        ) =>
        let _ = populateGravatarLoadAsEntityFunctions->Js.Array2.push(() => {
          let _ = InMemoryStore.Gravatar.getGravatar(~id=entityId)->Belt.Option.map(
            gravatarEntity => {
              switch uniqueUserDict->Js.Dict.get(gravatarEntity.owner) {
              | Some(_) => () // Already loaded
              | None =>
                let _ = uniqueUserAsEntityFieldArray->Js.Array2.push(gravatarEntity.owner)
                Js.Dict.set(uniqueUserDict, gravatarEntity.owner, gravatarEntity.owner)
              }
            },
          )
        })
      | None => ()
      }
    | NftcollectionRead(entityId) =>
      let _ = Js.Dict.set(uniqueNftcollectionDict, entityId, entityId)
    | TokenRead(entityId, tokenLoad) =>
      let _ = Js.Dict.set(uniqueTokenDict, entityId, entityId)
      switch tokenLoad.loadCollection {
      | Some(
          _ /* TODO: read this and recursively add loaders. See: https://github.com/Float-Capital/indexer/issues/293 */,
        ) =>
        let _ = populateTokenLoadAsEntityFunctions->Js.Array2.push(() => {
          let _ = InMemoryStore.Token.getToken(~id=entityId)->Belt.Option.map(
            tokenEntity => {
              switch uniqueNftcollectionDict->Js.Dict.get(tokenEntity.collection) {
              | Some(_) => () // Already loaded
              | None =>
                let _ =
                  uniqueNftcollectionAsEntityFieldArray->Js.Array2.push(tokenEntity.collection)
                Js.Dict.set(uniqueNftcollectionDict, tokenEntity.collection, tokenEntity.collection)
              }
            },
          )
        })
      | None => ()
      }
      switch tokenLoad.loadOwner {
      | Some(
          _ /* TODO: read this and recursively add loaders. See: https://github.com/Float-Capital/indexer/issues/293 */,
        ) =>
        let _ = populateTokenLoadAsEntityFunctions->Js.Array2.push(() => {
          let _ = InMemoryStore.Token.getToken(~id=entityId)->Belt.Option.map(
            tokenEntity => {
              switch uniqueUserDict->Js.Dict.get(tokenEntity.owner) {
              | Some(_) => () // Already loaded
              | None =>
                let _ = uniqueUserAsEntityFieldArray->Js.Array2.push(tokenEntity.owner)
                Js.Dict.set(uniqueUserDict, tokenEntity.owner, tokenEntity.owner)
              }
            },
          )
        })
      | None => ()
      }
    }
  })

  if Js.Dict.keys(uniqueUserDict)->Array.length > 0 {
    let userEntitiesArray =
      await sql->DbFunctions.User.readUserEntities(Js.Dict.values(uniqueUserDict))

    userEntitiesArray->Belt.Array.forEach(readRow => {
      let {entity, eventData} = DbFunctions.User.readRowToReadEntityData(readRow)
      InMemoryStore.User.setUser(~entity, ~eventData, ~crud=Types.Read)
    })
  }

  if Js.Dict.keys(uniqueGravatarDict)->Array.length > 0 {
    let gravatarEntitiesArray =
      await sql->DbFunctions.Gravatar.readGravatarEntities(Js.Dict.values(uniqueGravatarDict))

    gravatarEntitiesArray->Belt.Array.forEach(readRow => {
      let {entity, eventData} = DbFunctions.Gravatar.readRowToReadEntityData(readRow)
      InMemoryStore.Gravatar.setGravatar(~entity, ~eventData, ~crud=Types.Read)
    })
  }

  if Js.Dict.keys(uniqueNftcollectionDict)->Array.length > 0 {
    let nftcollectionEntitiesArray =
      await sql->DbFunctions.Nftcollection.readNftcollectionEntities(
        Js.Dict.values(uniqueNftcollectionDict),
      )

    nftcollectionEntitiesArray->Belt.Array.forEach(readRow => {
      let {entity, eventData} = DbFunctions.Nftcollection.readRowToReadEntityData(readRow)
      InMemoryStore.Nftcollection.setNftcollection(~entity, ~eventData, ~crud=Types.Read)
    })
  }

  if Js.Dict.keys(uniqueTokenDict)->Array.length > 0 {
    let tokenEntitiesArray =
      await sql->DbFunctions.Token.readTokenEntities(Js.Dict.values(uniqueTokenDict))

    tokenEntitiesArray->Belt.Array.forEach(readRow => {
      let {entity, eventData} = DbFunctions.Token.readRowToReadEntityData(readRow)
      InMemoryStore.Token.setToken(~entity, ~eventData, ~crud=Types.Read)
    })
  }

  // Execute first layer of additional load functions:
  // TODO: make this a recursive process
  populateUserLoadAsEntityFunctions->Belt.Array.forEach(func => func())
  populateGravatarLoadAsEntityFunctions->Belt.Array.forEach(func => func())
  populateNftcollectionLoadAsEntityFunctions->Belt.Array.forEach(func => func())
  populateTokenLoadAsEntityFunctions->Belt.Array.forEach(func => func())

  if uniqueUserAsEntityFieldArray->Array.length > 0 {
    let userFieldEntitiesArray =
      await sql->DbFunctions.User.readUserEntities(uniqueUserAsEntityFieldArray)

    userFieldEntitiesArray->Belt.Array.forEach(readRow => {
      let {entity, eventData} = DbFunctions.User.readRowToReadEntityData(readRow)
      InMemoryStore.User.setUser(~entity, ~eventData, ~crud=Types.Read)
    })
  }
  if uniqueGravatarAsEntityFieldArray->Array.length > 0 {
    let gravatarFieldEntitiesArray =
      await sql->DbFunctions.Gravatar.readGravatarEntities(uniqueGravatarAsEntityFieldArray)

    gravatarFieldEntitiesArray->Belt.Array.forEach(readRow => {
      let {entity, eventData} = DbFunctions.Gravatar.readRowToReadEntityData(readRow)
      InMemoryStore.Gravatar.setGravatar(~entity, ~eventData, ~crud=Types.Read)
    })
  }
  if uniqueNftcollectionAsEntityFieldArray->Array.length > 0 {
    let nftcollectionFieldEntitiesArray =
      await sql->DbFunctions.Nftcollection.readNftcollectionEntities(
        uniqueNftcollectionAsEntityFieldArray,
      )

    nftcollectionFieldEntitiesArray->Belt.Array.forEach(readRow => {
      let {entity, eventData} = DbFunctions.Nftcollection.readRowToReadEntityData(readRow)
      InMemoryStore.Nftcollection.setNftcollection(~entity, ~eventData, ~crud=Types.Read)
    })
  }
  if uniqueTokenAsEntityFieldArray->Array.length > 0 {
    let tokenFieldEntitiesArray =
      await sql->DbFunctions.Token.readTokenEntities(uniqueTokenAsEntityFieldArray)

    tokenFieldEntitiesArray->Belt.Array.forEach(readRow => {
      let {entity, eventData} = DbFunctions.Token.readRowToReadEntityData(readRow)
      InMemoryStore.Token.setToken(~entity, ~eventData, ~crud=Types.Read)
    })
  }
}

let executeBatch = async sql => {
  let rawEventsRows = InMemoryStore.RawEvents.rawEventsDict.contents->Js.Dict.values

  let deleteRawEventsIdsPromise = sql => {
    let deleteRawEventsIds =
      rawEventsRows
      ->Belt.Array.keepMap(rawEventsRow =>
        rawEventsRow.crud == Types.Delete ? Some(rawEventsRow.entity) : None
      )
      ->Belt.Array.map(rawEvents => (rawEvents.chainId, rawEvents.eventId))

    if deleteRawEventsIds->Belt.Array.length > 0 {
      sql->DbFunctions.RawEvents.batchDeleteRawEvents(deleteRawEventsIds)
    } else {
      ()->Promise.resolve
    }
  }

  let setRawEventsPromise = sql => {
    let setRawEvents =
      rawEventsRows->Belt.Array.keepMap(rawEventsRow =>
        rawEventsRow.crud == Types.Create || rawEventsRow.crud == Update
          ? Some(rawEventsRow.entity)
          : None
      )

    if setRawEvents->Belt.Array.length > 0 {
      sql->DbFunctions.RawEvents.batchSetRawEvents(setRawEvents)
    } else {
      ()->Promise.resolve
    }
  }

  let dynamicContractRegistryRows =
    InMemoryStore.DynamicContractRegistry.dynamicContractRegistryDict.contents->Js.Dict.values

  let deleteDynamicContractRegistryIdsPromise = sql => {
    let deleteDynamicContractRegistryIds =
      dynamicContractRegistryRows
      ->Belt.Array.keepMap(dynamicContractRegistryRow =>
        dynamicContractRegistryRow.crud == Types.Delete
          ? Some(dynamicContractRegistryRow.entity)
          : None
      )
      ->Belt.Array.map(dynamicContractRegistry => (
        dynamicContractRegistry.chainId,
        dynamicContractRegistry.contractAddress,
      ))

    if deleteDynamicContractRegistryIds->Belt.Array.length > 0 {
      sql->DbFunctions.DynamicContractRegistry.batchDeleteDynamicContractRegistry(
        deleteDynamicContractRegistryIds,
      )
    } else {
      ()->Promise.resolve
    }
  }

  let setDynamicContractRegistryPromise = sql => {
    let setDynamicContractRegistry =
      dynamicContractRegistryRows->Belt.Array.keepMap(dynamicContractRegistryRow =>
        dynamicContractRegistryRow.crud == Types.Create || dynamicContractRegistryRow.crud == Update
          ? Some(dynamicContractRegistryRow.entity)
          : None
      )

    if setDynamicContractRegistry->Belt.Array.length > 0 {
      sql->DbFunctions.DynamicContractRegistry.batchSetDynamicContractRegistry(
        setDynamicContractRegistry,
      )
    } else {
      ()->Promise.resolve
    }
  }

  let userRows = InMemoryStore.User.userDict.contents->Js.Dict.values

  let deleteUserIdsPromise = sql => {
    let deleteUserIds =
      userRows
      ->Belt.Array.keepMap(userRow => userRow.crud == Types.Delete ? Some(userRow.entity) : None)
      ->Belt.Array.map(user => user.id)

    if deleteUserIds->Belt.Array.length > 0 {
      sql->DbFunctions.User.batchDeleteUser(deleteUserIds)
    } else {
      ()->Promise.resolve
    }
  }
  let setUserPromise = sql => {
    let setUser = userRows->Belt.Array.keepMap(userRow =>
      userRow.crud == Types.Create || userRow.crud == Update
        ? Some({
            ...userRow,
            entity: userRow.entity->Types.serializeUserEntity,
          })
        : None
    )

    if setUser->Belt.Array.length > 0 {
      sql->DbFunctions.User.batchSetUser(setUser)
    } else {
      ()->Promise.resolve
    }
  }

  let gravatarRows = InMemoryStore.Gravatar.gravatarDict.contents->Js.Dict.values

  let deleteGravatarIdsPromise = sql => {
    let deleteGravatarIds =
      gravatarRows
      ->Belt.Array.keepMap(gravatarRow =>
        gravatarRow.crud == Types.Delete ? Some(gravatarRow.entity) : None
      )
      ->Belt.Array.map(gravatar => gravatar.id)

    if deleteGravatarIds->Belt.Array.length > 0 {
      sql->DbFunctions.Gravatar.batchDeleteGravatar(deleteGravatarIds)
    } else {
      ()->Promise.resolve
    }
  }
  let setGravatarPromise = sql => {
    let setGravatar = gravatarRows->Belt.Array.keepMap(gravatarRow =>
      gravatarRow.crud == Types.Create || gravatarRow.crud == Update
        ? Some({
            ...gravatarRow,
            entity: gravatarRow.entity->Types.serializeGravatarEntity,
          })
        : None
    )

    if setGravatar->Belt.Array.length > 0 {
      sql->DbFunctions.Gravatar.batchSetGravatar(setGravatar)
    } else {
      ()->Promise.resolve
    }
  }

  let nftcollectionRows = InMemoryStore.Nftcollection.nftcollectionDict.contents->Js.Dict.values

  let deleteNftcollectionIdsPromise = sql => {
    let deleteNftcollectionIds =
      nftcollectionRows
      ->Belt.Array.keepMap(nftcollectionRow =>
        nftcollectionRow.crud == Types.Delete ? Some(nftcollectionRow.entity) : None
      )
      ->Belt.Array.map(nftcollection => nftcollection.id)

    if deleteNftcollectionIds->Belt.Array.length > 0 {
      sql->DbFunctions.Nftcollection.batchDeleteNftcollection(deleteNftcollectionIds)
    } else {
      ()->Promise.resolve
    }
  }
  let setNftcollectionPromise = sql => {
    let setNftcollection = nftcollectionRows->Belt.Array.keepMap(nftcollectionRow =>
      nftcollectionRow.crud == Types.Create || nftcollectionRow.crud == Update
        ? Some({
            ...nftcollectionRow,
            entity: nftcollectionRow.entity->Types.serializeNftcollectionEntity,
          })
        : None
    )

    if setNftcollection->Belt.Array.length > 0 {
      sql->DbFunctions.Nftcollection.batchSetNftcollection(setNftcollection)
    } else {
      ()->Promise.resolve
    }
  }

  let tokenRows = InMemoryStore.Token.tokenDict.contents->Js.Dict.values

  let deleteTokenIdsPromise = sql => {
    let deleteTokenIds =
      tokenRows
      ->Belt.Array.keepMap(tokenRow => tokenRow.crud == Types.Delete ? Some(tokenRow.entity) : None)
      ->Belt.Array.map(token => token.id)

    if deleteTokenIds->Belt.Array.length > 0 {
      sql->DbFunctions.Token.batchDeleteToken(deleteTokenIds)
    } else {
      ()->Promise.resolve
    }
  }
  let setTokenPromise = sql => {
    let setToken = tokenRows->Belt.Array.keepMap(tokenRow =>
      tokenRow.crud == Types.Create || tokenRow.crud == Update
        ? Some({
            ...tokenRow,
            entity: tokenRow.entity->Types.serializeTokenEntity,
          })
        : None
    )

    if setToken->Belt.Array.length > 0 {
      sql->DbFunctions.Token.batchSetToken(setToken)
    } else {
      ()->Promise.resolve
    }
  }

  let res = await sql->Postgres.beginSql(sql => {
    [
      sql->deleteRawEventsIdsPromise,
      sql->setRawEventsPromise,
      sql->deleteDynamicContractRegistryIdsPromise,
      sql->setDynamicContractRegistryPromise,
      sql->deleteUserIdsPromise,
      sql->setUserPromise,
      sql->deleteGravatarIdsPromise,
      sql->setGravatarPromise,
      sql->deleteNftcollectionIdsPromise,
      sql->setNftcollectionPromise,
      sql->deleteTokenIdsPromise,
      sql->setTokenPromise,
    ]
  })

  res
}
