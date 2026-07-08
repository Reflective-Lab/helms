# Truth: Overdue balance blocks entitlement increase
@truth @policy @revenue
Feature: Overdue balance blocks entitlement increase

  Intent:
    Outcome: block entitlement expansion while customer obligations are overdue

  Scenario: Customer asks for more access while overdue
    Given a customer has overdue financial obligations
    When the system evaluates an entitlement increase
    Then the increase shall be blocked by default
    And any temporary relief shall be time-bound
    And exceptions shall move through an explicit approval path
