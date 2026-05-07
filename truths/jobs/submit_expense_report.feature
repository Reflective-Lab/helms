Feature: Submit expense report
  As an employee
  I want to submit a reimbursable expense report with receipt evidence
  So that finance can review it and export it into bookkeeping

  Intent:
    Outcome: submit employee expense report for reimbursement

  Scenario: Expense report is submitted with review-ready evidence
    Given an employee has one or more reimbursable expenses
    And each claimed amount is attached to receipt evidence
    When the employee submits the expense report
    Then the expense report is recorded with attributable evidence
    And the approval route is explicit
    And the export status is queryable

  Scenario: OCR or policy ambiguity opens a human gate
    Given an expense line has low-confidence OCR or falls outside policy
    When the expense report is submitted
    Then the report is blocked for manual review
    And the blocking reason is explicit
