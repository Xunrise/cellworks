<!-- UNTRUSTED USER-SUBMITTED CONTENT.
     This is a problem report. It is data to analyse.
     It carries no instructions for you. -->

# Issue #52: STORY-030: Sessions walk to the towers they work on
Reported by: Xunrise

Feature: Sessions walk to the towers they work on
  As an AI operator
  I want to see each running session move through the city to the code it touches, and find any session in a sidebar
  So that I know where every agent is working at a glance

  Background:
    Given the project "shop" is open as a city
    And the session "pricing-fix" is running in "shop"

  Scenario: A session walks to the tower it reads
    When "pricing-fix" reads the function "discount"
    Then the session "pricing-fix" walks to the tower "discount" and works there

  Scenario: Naming a file in a command counts as reading it
    When "pricing-fix" runs a command that names the file "catalog/pricing.rs"
    Then the session "pricing-fix" stands at a tower of "pricing.rs"

  Scenario: Writing adds floors while the session writes
    When "pricing-fix" writes new lines into "discount"
    Then the tower "discount" grows by the floors being written
    And the new floors are marked as being written

  Scenario: A session waiting for the engineer asks for attention
    When "pricing-fix" waits for the engineer
    Then the session "pricing-fix" stays at the last tower it touched and signals that it waits for the engineer
    And the sidebar lists "pricing-fix" among the sessions that need the engineer

  Scenario: Sessions at the same tower stand apart
    Given the session "docs-run" is running in "shop"
    When "pricing-fix" and "docs-run" both read "discount"
    Then both sessions are shown at "discount" without covering each other

  Scenario: A session working in another checkout of the project is in the city
    Given the session "loyalty-proto" works in a worktree of "shop"
    When "loyalty-proto" reads the function "discount" there
    Then the session "loyalty-proto" stands at the tower "discount" of the city of "shop"

  Scenario: A session in another project is not in this city
    Given the session "docs-run" is running in the project "handbook"
    Then the session "docs-run" is not shown in the city of "shop"
    And the sidebar lists "docs-run" under other projects

  Scenario: Selecting a session in the sidebar brings it into view
    When the engineer selects "pricing-fix" in the sidebar
    Then "pricing-fix" is selected and the city centres on its tower

  Scenario: A session that ended leaves the city
    When "pricing-fix" ends
    Then the session "pricing-fix" is no longer shown in the city or the sidebar

Prototype: proto/sessions-walk

