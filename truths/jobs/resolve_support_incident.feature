# Truth: Resolve support incident
@truth @job @support
Feature: Resolve support incident

  Intent:
    Outcome: drive a customer issue from intake through diagnosis to verified resolution or escalation

  Scenario: A customer issue is diagnosed and closed
    Given a customer issue has been captured in a conversation thread
    When support and subject-matter experts work the case
    Then tasks and workflow state shall track the handoffs
    And evidence for diagnosis shall be attached to the case
    And the customer-facing resolution shall be explicit before closure
    And unresolved risk shall trigger escalation
