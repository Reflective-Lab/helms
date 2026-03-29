# Truth: Promoted fact requires traceable evidence
@truth @policy @trust
Feature: Promoted fact requires traceable evidence

  Scenario: A proposed fact is promoted to durable truth
    Given a proposed fact exists in customer context
    When the system attempts to promote it
    Then the fact shall link to traceable evidence
    And low-confidence promotions shall require explicit review
    And provenance for the promotion shall remain immutable
