Feature: Match renewal context
  Scenario: Assemble a renewal brief from account memory
    Given a customer account has conversations, documents, and facts
    When the system retrieves renewal-relevant context
    Then a renewal brief shall be attached to the account
    And renewal signals shall remain traceable to their sources
