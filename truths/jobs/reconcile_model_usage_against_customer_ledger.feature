# Truth: Reconcile model usage against customer ledger
@truth @job @finance
Feature: Reconcile model usage against customer ledger

  Intent:
    Outcome: align usage metering, financial balance, and entitlement burn-down without mutating history

  Scenario: Usage and financial balance must agree
    Given metered model usage exists for a customer period
    When reconciliation is executed
    Then usage totals shall be compared with ledger movements
    And entitlement burn-down shall match the usable balance
    And unreconciled deltas above threshold shall require review
    And all adjustments shall remain auditable
