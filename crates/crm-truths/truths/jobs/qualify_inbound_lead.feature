# Truth: Qualify inbound lead
Feature: Qualify inbound lead

  Intent:
    Outcome: qualify inbound lead with external enrichment and governed routing

  @truth @job @commercial
  Scenario: Inbound interest is assessed for fit
    Given an inbound contact arrives from a known or new organization
    When the operator captures the conversation and qualification facts
    Then the account and contact graph shall be linked to the lead
    And the lead shall end in qualified or disqualified state
    And the next owner and next step shall be explicit
    And the rationale shall remain attributable to evidence
