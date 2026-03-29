# Truth: Qualify inbound lead
@truth @job @commercial
Feature: Qualify inbound lead

  Scenario: Inbound interest is assessed for fit
    Given an inbound contact arrives from a known or new organization
    When the operator captures the conversation and qualification facts
    Then the account and contact graph shall be linked to the lead
    And the lead shall end in qualified or disqualified state
    And the next owner and next step shall be explicit
    And the rationale shall remain attributable to evidence
