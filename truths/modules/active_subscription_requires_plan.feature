# Truth: Active subscription requires plan
@truth @module @subscriptions
Feature: Active subscription requires plan

  Scenario: Activating a subscription
    Given a subscription is about to move to active state
    When activation is attempted
    Then the subscription shall resolve to a valid catalog plan
    And the entitlement source for that plan shall be explicit
    And activation without a valid plan shall be rejected
