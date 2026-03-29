# Truth: Suspend service on payment failure
@truth @job @revenue
Feature: Suspend service on payment failure

  Scenario: Failed payment triggers controlled service suspension
    Given a customer payment is failed or overdue beyond policy
    When the suspension job is evaluated
    Then grace-period rules shall be checked before suspension
    And subscription state shall move to the correct suspended status
    And entitlements shall reflect that status
    And strategic account overrides shall require approval
