Feature: Score inbound fit

  Intent:
    Outcome: score inbound lead fit using website behavior and inbound context

  Scenario: Score a new inbound lead from website behavior
    Given a lead has attributable website usage events
    And the inbound account context is known
    When the system extracts behavioral features and evaluates fit
    Then a fit score shall be recorded for the lead
    And the score shall cite the behavioral evidence used

