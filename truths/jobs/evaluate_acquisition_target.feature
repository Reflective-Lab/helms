Feature: Evaluate acquisition target

  As a deal lead
  I want to converge multi-source evidence into a structured acquisition recommendation
  So that the investment committee can make a go/no-go decision with traceable evidence

  Scenario: A target company is evaluated with convergent due diligence
    Given a target company has been identified for acquisition
    When due diligence research converges
    Then a recommendation shall be produced with confidence at least 0.7
    And all material contradictions shall be surfaced and documented
    And each DD dimension shall cite at least one independent source

  Scenario: Contradictions require human review before recommendation
    Given due diligence research has surfaced contradictory claims
    When the synthesis attempts to produce a recommendation
    Then the recommendation shall be blocked until contradictions are reviewed
    And the contradictions shall be presented with both claims and their sources

  Scenario: Investment committee approval is required
    Given a recommendation has been produced
    When the recommendation is ready for delivery
    Then investment committee approval shall be required before the recommendation leaves draft
    And the approval decision shall be recorded with rationale
