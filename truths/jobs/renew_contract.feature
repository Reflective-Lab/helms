# Truth: Renew contract
@truth @job @commercial
Feature: Renew contract

  Scenario: A customer is brought to a new commercial term
    Given a customer has an active commercial relationship nearing renewal
    When the renewal job is executed
    Then the account and stakeholder context shall be current
    And the proposed commercial terms shall resolve to catalog offers
    And the current proposal or contract version shall be attached
    And non-standard terms shall require approval
