# Truth: Top-up requires confirmed payment
@truth @policy @revenue
Feature: Top-up requires confirmed payment

  Scenario: Applying a prepaid credit grant
    Given a top-up purchase exists
    When the system attempts to increase customer balance
    Then the payment shall already be confirmed
    And an unconfirmed payment shall block the credit grant
    And any manual override shall be explicitly approved and audited
