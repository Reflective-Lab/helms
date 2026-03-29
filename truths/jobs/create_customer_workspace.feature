# Truth: Create customer workspace
@truth @job @provisioning
Feature: Create customer workspace

  Scenario: A sold customer needs an operational workspace
    Given a customer account has an active commercial commitment
    When the system provisions a new workspace
    Then the workspace shall be linked to the correct customer account
    And the purchased plan shall be attached to the workspace
    And entitlements shall be initialized from that plan
    And non-standard provisioning shall require explicit approval
