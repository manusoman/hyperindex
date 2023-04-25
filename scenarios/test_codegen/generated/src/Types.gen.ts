/* TypeScript file generated from Types.res by genType. */
/* eslint-disable import/first */


import type {BigInt_t as Ethers_BigInt_t} from '../src/bindings/Ethers.gen';

import type {ethAddress as Ethers_ethAddress} from '../src/bindings/Ethers.gen';

// tslint:disable-next-line:interface-over-type-literal
export type id = string;
export type Id = id;

// tslint:disable-next-line:interface-over-type-literal
export type contactDetails = { readonly name: string; readonly email: string };

// tslint:disable-next-line:interface-over-type-literal
export type userEntity = {
  readonly id: string; 
  readonly address: string; 
  readonly gravatar: (undefined | id)
};

// tslint:disable-next-line:interface-over-type-literal
export type gravatarEntity = {
  readonly id: string; 
  readonly owner: id; 
  readonly displayName: string; 
  readonly imageUrl: string; 
  readonly updatesCount: number
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_TestEventEvent_eventArgs = {
  readonly id: Ethers_BigInt_t; 
  readonly user: Ethers_ethAddress; 
  readonly contactDetails: contactDetails
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_TestEventEvent_userEntityHandlerContext = {
  readonly insert: (_1:userEntity) => void; 
  readonly update: (_1:userEntity) => void; 
  readonly delete: (_1:id) => void
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_TestEventEvent_gravatarEntityHandlerContext = {
  readonly insert: (_1:gravatarEntity) => void; 
  readonly update: (_1:gravatarEntity) => void; 
  readonly delete: (_1:id) => void
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_TestEventEvent_context = { readonly user: GravatarContract_TestEventEvent_userEntityHandlerContext; readonly gravatar: GravatarContract_TestEventEvent_gravatarEntityHandlerContext };

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_TestEventEvent_loaderContext = {};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_NewGravatarEvent_eventArgs = {
  readonly id: Ethers_BigInt_t; 
  readonly owner: Ethers_ethAddress; 
  readonly displayName: string; 
  readonly imageUrl: string
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_NewGravatarEvent_userEntityHandlerContext = {
  readonly insert: (_1:userEntity) => void; 
  readonly update: (_1:userEntity) => void; 
  readonly delete: (_1:id) => void
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_NewGravatarEvent_gravatarEntityHandlerContext = {
  readonly insert: (_1:gravatarEntity) => void; 
  readonly update: (_1:gravatarEntity) => void; 
  readonly delete: (_1:id) => void
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_NewGravatarEvent_context = { readonly user: GravatarContract_NewGravatarEvent_userEntityHandlerContext; readonly gravatar: GravatarContract_NewGravatarEvent_gravatarEntityHandlerContext };

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_NewGravatarEvent_loaderContext = {};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_UpdatedGravatarEvent_eventArgs = {
  readonly id: Ethers_BigInt_t; 
  readonly owner: Ethers_ethAddress; 
  readonly displayName: string; 
  readonly imageUrl: string
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_UpdatedGravatarEvent_userEntityHandlerContext = {
  readonly insert: (_1:userEntity) => void; 
  readonly update: (_1:userEntity) => void; 
  readonly delete: (_1:id) => void
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_UpdatedGravatarEvent_gravatarEntityHandlerContext = {
  readonly gravatarWithChanges: () => (undefined | gravatarEntity); 
  readonly insert: (_1:gravatarEntity) => void; 
  readonly update: (_1:gravatarEntity) => void; 
  readonly delete: (_1:id) => void
};

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_UpdatedGravatarEvent_context = { readonly user: GravatarContract_UpdatedGravatarEvent_userEntityHandlerContext; readonly gravatar: GravatarContract_UpdatedGravatarEvent_gravatarEntityHandlerContext };

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_UpdatedGravatarEvent_gravatarEntityLoaderContext = { readonly gravatarWithChangesLoad: (_1:id) => void };

// tslint:disable-next-line:interface-over-type-literal
export type GravatarContract_UpdatedGravatarEvent_loaderContext = { readonly gravatar: GravatarContract_UpdatedGravatarEvent_gravatarEntityLoaderContext };
