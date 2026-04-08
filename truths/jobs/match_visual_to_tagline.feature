Feature: Match visual to tagline
  Scenario: Governed pairing slate from a campaign brief
    Given a CampaignBrief with audience, channels, and constraints
    And a brand-safe asset library is reachable via the DAM port
    And brand guardrail facts exist for the party
    When the match truth activates
    Then visual candidates shall be retrieved with provenance
    And tagline candidates shall be drafted in the storyteller voice
    And every visual and tagline pair shall carry brand, audience, and narrative scores
    And a ranked PairingSlate shall be proposed
    But no pairing shall be promoted without the required human approvals

  Scenario: Multi-role approval gates a single pairing
    Given a ranked PairingSlate has been proposed
    When the top pairing is routed for review
    Then the brand-manager shall approve brand fit
    And the marketer shall approve audience fit
    And the storyteller shall approve narrative and copy
    And only after all required approvals shall the CampaignPairing fact be promoted

  Scenario: Risk flag triggers legal approval
    Given the risk screen raises a claims or IP flag on a pairing
    When that pairing is routed for review
    Then a legal review request shall be opened via the legal intake port
    And the pairing shall not be promoted without a recorded legal approval fact

  Scenario: Synthetic visual candidates are never auto-selected
    Given the image generation provider is enabled for this brief
    When synthetic visuals are proposed alongside DAM visuals
    Then every synthetic candidate shall be tagged as synthetic
    And a synthetic candidate shall require explicit human selection before promotion
