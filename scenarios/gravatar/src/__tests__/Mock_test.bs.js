// Generated by ReScript, PLEASE EDIT WITH CARE
'use strict';

var Jest = require("@glennsl/rescript-jest/src/jest.bs.js");
var DbStub = require("./__mocks__/DbStub.bs.js");
var Belt_Array = require("rescript/lib/js/belt_Array.js");
var MockEvents = require("./__mocks__/MockEvents.bs.js");
var ContextMock = require("./__mocks__/ContextMock.bs.js");
var MockEntities = require("./__mocks__/MockEntities.bs.js");
var EventProcessing = require("generated/src/EventProcessing.bs.js");

require("../EventHandlers.bs.js")
;

Jest.describe("E2E Mock Event Batch", (function (param) {
        beforeAll(function () {
              DbStub.setGravatarDb(MockEntities.gravatarEntity1);
              DbStub.setGravatarDb(MockEntities.gravatarEntity2);
              return Belt_Array.forEach(MockEvents.eventBatchWithContext, EventProcessing.eventRouter);
            });
        afterAll(function () {
              ContextMock.insertMock.mockClear();
              ContextMock.updateMock.mockClear();
            });
        Jest.test("3 newgravatar event insert calls in order", (function (param) {
                var insertCalls = Jest.MockJs.calls(ContextMock.insertMock);
                return Jest.Expect.toEqual(Jest.Expect.expect(insertCalls), [
                            MockEvents.newGravatar1.id.toString(),
                            MockEvents.newGravatar2.id.toString(),
                            MockEvents.newGravatar3.id.toString()
                          ]);
              }));
        Jest.test("3 updategravatar event insert calls in order", (function (param) {
                var insertCalls = Jest.MockJs.calls(ContextMock.insertMock);
                return Jest.Expect.toEqual(Jest.Expect.expect(insertCalls), [
                            MockEvents.updatedGravatar1.id.toString(),
                            MockEvents.updatedGravatar2.id.toString(),
                            MockEvents.updatedGravatar3.id.toString()
                          ]);
              }));
      }));

Jest.describe("E2E Db check", (function (param) {
        Jest.test("False test! Replace with real db test once drizzle is removed", (function (param) {
                return Jest.pass;
              }));
      }));

/*  Not a pure module */
