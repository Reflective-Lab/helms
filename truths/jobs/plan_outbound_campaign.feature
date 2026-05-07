Feature: Plan outbound campaign

  Intent:
    Outcome: plan outbound campaign with governed spend and downstream customer handoff

  Scenario: Assign prospects to reps within campaign guardrails
    Given a campaign has a pool of prospects and available reps
    And a campaign budget is defined
    When the system computes an outbound plan
    Then a governed campaign plan shall be produced
    And the budget status shall be explicit

