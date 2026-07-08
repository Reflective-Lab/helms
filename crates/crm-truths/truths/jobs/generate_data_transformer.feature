Feature: Generate data transformer

  As an operator
  I want the system to generate a verified data transformer when an existing one doesn't fit
  So that the convergence loop can continue without manual coding

  Scenario: A convergence loop generates code when it discovers a gap
    Given a convergence loop has identified a data transformation need
    And no existing transformer handles the required format
    When the code generation suggestor produces a transformer
    Then the generated code shall compile to valid Wasm
    And the generated module shall pass acceptance test cases
    And the verified artifact shall be promoted as a fact with provenance

  Scenario: Failed verification triggers retry with feedback
    Given a code generation suggestor has produced a transformer
    And the verifier detects a compilation or test failure
    When the failure is recorded as an evaluation fact
    Then the gap detector shall propose a refined generation strategy
    And the code generation suggestor shall retry with failure context

  Scenario: Generated artifact carries full provenance
    Given a verified transformer has been promoted
    Then the artifact fact shall cite the generation prompt
    And the artifact fact shall cite the verification result
    And the artifact fact shall include a content hash
