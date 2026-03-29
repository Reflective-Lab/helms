# Truth: Detect abnormal token burn
@truth @job @monitoring
Feature: Detect abnormal token burn

  Scenario: Unexpected usage surge requires intervention
    Given recent token usage materially exceeds expected patterns
    When the anomaly job evaluates the customer context
    Then the system shall cite telemetry supporting the anomaly
    And a workflow case shall be opened for mitigation
    And recommended actions shall be recorded with agent provenance
    And hard-limit intervention shall require approval
