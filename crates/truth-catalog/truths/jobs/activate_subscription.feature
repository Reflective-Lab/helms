# Truth: Activate subscription
@truth @job @commercial
Feature: Activate subscription

  Intent:
    Outcome: activate subscription and entitlement state from an agreed commercial plan

  Scenario: A commercial commitment becomes active
    Given a customer has accepted a valid plan
    When the subscription is activated
    Then the subscription shall reference a catalog plan
    And entitlements shall be derived from that plan
    And the financial opening state shall be auditable
    And activation exceptions shall move through a workflow case
