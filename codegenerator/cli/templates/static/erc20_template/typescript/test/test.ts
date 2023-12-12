import assert from "assert";
import { MockDb, ERC20 } from "../generated/src/TestHelpers.gen";
import { AccountEntity } from "../generated/src/Types.gen";
import { Addresses } from "../generated/src/bindings/Ethers.gen";

describe("Transfers", () => {
  it("Transfer subtracts the from account balance and adds to the to account balance", () => {
    //Instantiate a mock DB
    const mockDb = MockDb.createMockDb();

    //Get mock addresses from helpers
    const userAddress1 = Addresses.mockAddresses[0];
    const userAddress2 = Addresses.mockAddresses[1];

    //Make a mock entity to set the initial state of the mock db
    const mockAccountEntity: AccountEntity = {
      id: userAddress1,
      balance: 5n,
    };

    //Set an initial state for the user
    mockDb.entities.Account.set(mockAccountEntity);

    //Create a mock Transfer event from userAddress1 to userAddress2
    const mockTransfer = ERC20.Transfer.createMockEvent({
      from: userAddress1,
      to: userAddress2,
      value: 3n,
    });

    //Process the mockEvent
    //This takes in the mockDb and returns a new updated mockDb.
    //The initial mockDb is not mutated with processEvent
    const mockDbAfterTransfer = ERC20.Transfer.processEvent({
      event: mockTransfer,
      mockDb,
    });

    //Get the balance of userAddress1 after the transfer
    const account1Balance =
      mockDbAfterTransfer.entities.Account.get(userAddress1)?.balance;

    //Assert the expected balance
    assert.equal(
      2n,
      account1Balance,
      "Should have subtracted transfer amount 3 from userAddress1 balance 5",
    );

    //Get the balance of userAddress2 after the transfer
    const account2Balance =
      mockDbAfterTransfer.entities.Account.get(userAddress2)?.balance;

    //Assert the expected balance
    assert.equal(
      3n,
      account2Balance,
      "Should have added transfer amount 3 to userAddress2 balance 0",
    );
  });
});
