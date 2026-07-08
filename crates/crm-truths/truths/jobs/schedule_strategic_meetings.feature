Feature: Schedule strategic meetings

  Intent:
    Outcome: rank prospects by strategy alignment, resolve availability, and propose a concrete meeting slate with reasoning

  Scenario: Book meetings with highest-value prospects from free-text intent
    Given a free-text scheduling intent and a time window
    And the scored pipeline of qualified leads
    And the active strategy and campaign context
    And the actor's calendar availability
    When the truth activates
    Then a ranked meeting slate shall be proposed with reasoning
    And each proposed meeting shall cite strategy alignment evidence
    But no meeting shall be auto-booked without human confirmation
