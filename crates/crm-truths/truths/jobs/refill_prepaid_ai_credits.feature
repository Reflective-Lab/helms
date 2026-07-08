# Truth: Refill prepaid AI credits
@truth @job @revenue
Feature: Refill prepaid AI credits

  Intent:
    Outcome: apply top-up purchase to prepaid AI credit balances with financial traceability

  Scenario: Customer tops up after balance exhaustion
    Given a customer has an active subscription
    And the customer prepaid balance is exhausted
    When the customer purchases a top-up package
    Then the payment shall be confirmed before balance changes
    And the ledger shall record a credit grant
    And the entitlement balance shall increase for the correct account
