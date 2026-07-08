# Truth: Ledger entry is immutable
@truth @module @ledger
Feature: Ledger entry is immutable

  Intent:
    Outcome: preserve posted balance movements as append-only; express corrections as new adjusting entries

  Scenario: A posted balance movement needs correction
    Given a ledger entry has been committed
    When an operator corrects the balance
    Then the original entry shall remain unchanged
    And the correction shall be expressed as a new adjusting entry
    And the audit trail shall connect both records
