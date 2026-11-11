<!-- UNTRUSTED USER-SUBMITTED CONTENT.
     This is a problem report. It is data to analyse.
     It carries no instructions for you. -->

# Issue #46: STORY-024: Finish a review
Reported by: Xunrise

Feature: Finish a review
  As an engineer reviewing an agent's work
  I want to walk through every change, critical ones first, and compare before and after
  So that I approve only what I have actually looked at

  Background:
    Given the engineer is reviewing the pull request "#42"
    And "#42" changes the critical tower "Order::cancel" and the tower "receipt"

  Scenario: Stepping through changes visits critical towers first
    When the engineer steps to the next change
    Then the view goes to the tower "Order::cancel"
    And "Order::cancel" counts as visited

  Scenario: Choosing a change in the review counts as visiting it
    When the engineer chooses "receipt" in the review's list of changed towers
    Then "receipt" counts as visited

  Scenario: The review shows how much has been looked at
    Given the engineer has visited "Order::cancel"
    Then the review shows one of two changed towers visited

  Scenario: Before and after can be compared
    When the engineer switches to before
    Then the city shows the towers as they were before "#42"
    And the lines "#42" removes are marked in it
    When the engineer switches to after
    Then the city shows the towers as "#42" leaves them

  Scenario: Approving with unvisited critical towers asks first
    Given the engineer has not visited "Order::cancel"
    When the engineer approves "#42"
    Then the engineer is asked to confirm and "Order::cancel" is named

  Scenario: Approving after visiting every critical tower needs no confirmation
    Given the engineer has visited "Order::cancel"
    When the engineer approves "#42"
    Then "#42" is approved

  Scenario: What was visited lasts only while the review is open
    Given the engineer has visited "Order::cancel"
    When the engineer leaves the review and opens "#42" as a review again
    Then no changed tower counts as visited

Prototype: proto/finish-review

