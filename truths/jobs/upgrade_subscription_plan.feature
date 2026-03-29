# Truth: Upgrade subscription plan
@truth @job @commercial
Feature: Upgrade subscription plan

  Scenario: Customer moves to a better commercial plan
    Given a customer has an active subscription
    When the customer accepts an upgrade
    Then the target plan shall exist in the catalog
    And the effective date shall be explicit
    And entitlements shall be moved to the new plan
    And non-standard pricing shall require approval
