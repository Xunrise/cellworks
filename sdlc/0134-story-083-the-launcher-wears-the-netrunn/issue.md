<!-- UNTRUSTED USER-SUBMITTED CONTENT.
     This is a problem report. It is data to analyse.
     It carries no instructions for you. -->

# Issue #134: STORY-083: The launcher wears the Netrunner look with switchable theme words
Reported by: Xunrise

Feature: The launcher wears the Netrunner look with switchable theme words
  As an engineer opening Cellworks
  I want the launcher in the Netrunner look and its theme words
  So that the first screen matches the rest of the Netrunner map
  And I can still switch to plain words when I prefer them

  Background:
    Given no project is open
    And the local machine has the projects "cellworks" and "The-Factory"
    And the remote host "wsl" is connected and has the project "factory-evals"
    And the remote host "devbox" is offline and has the folder "surface-kit"

  Scenario: Theme words are shown by default
    Given the settings do not mention theme words
    When the launcher opens
    Then the header reads "4 fortresses · 2 nets"
    And the location chips read "All 4", "Local net 2", "wsl connected 1" and "devbox offline 1"
    And the open action for "cellworks" reads "Open fortress"

  Scenario: An offline host is reported as ICE
    Given the settings do not mention theme words
    When the engineer selects "surface-kit"
    Then the preview warns "ICE // NET OFFLINE"
    And the open action reads "Connect the net first" and cannot be used

  Scenario: Theme words can be switched off
    Given the settings switch theme words off
    When the launcher opens
    Then the header reads "4 projects · 2 hosts"
    And the location chips read "All 4", "This Mac 2", "wsl connected 1" and "devbox offline 1"
    And the open action for "cellworks" reads "Open project"

  Scenario: Plain words for an offline host
    Given the settings switch theme words off
    When the engineer selects "surface-kit"
    Then the preview warns "Host offline"
    And the open action reads "Connect the host first" and cannot be used

  Scenario: Switching words does not change behaviour
    Given the settings switch theme words off
    When the engineer searches for "fac"
    Then the same three projects are listed as with theme words on
    And Enter on "cellworks" opens it

Prototype: proto/launcher-netrunner

