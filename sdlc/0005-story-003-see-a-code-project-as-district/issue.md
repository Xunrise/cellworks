<!-- UNTRUSTED USER-SUBMITTED CONTENT.
     This is a problem report. It is data to analyse.
     It carries no instructions for you. -->

# Issue #5: STORY-003: See a code project as districts, sub-districts, groups and nodes
Reported by: Xunrise

Feature: See a code project as districts, sub-districts, groups and nodes
  As a code architect
  I want a code project's folders laid out as districts, sub-districts, groups and nodes
  So that I can read its structure at a glance

  # Scope: the layout is a simple, deterministic arrangement that keeps every
  # district, sub-district and group apart; the physics layout replaces it in
  # EPIC-002. Districts are cut by directories, the current district strategy.
  # Which common prefix is stripped before the districts is a per-project
  # setting, "district_prefix" in the settings file: "one-wrapper-folder" (the
  # default), "longest-shared-prefix" or "none". Product owner, 2026-10-09.

  Scenario: By default, top folders below the common prefix become districts
    Given a code project whose files all live under "src", in the folders "catalog" and "ordering"
    When the user opens the project
    Then the project shows the districts "catalog" and "ordering"

  Scenario: The next folder level becomes sub-districts, deeper folders become groups
    Given a code project with the files "src/catalog/pricing/rules/discount.rs" and "src/catalog/pricing/price.rs"
    When the user opens the project
    Then the district "catalog" holds the sub-district "pricing"
    And the sub-district "pricing" holds the group "rules" with the node "discount.rs"
    And the sub-district "pricing" holds the node "price.rs"

  Scenario: Files directly in a district are its nodes
    Given a code project with the files "src/catalog/item.rs" and "src/ordering/order.rs"
    When the user opens the project
    Then the district "catalog" holds the node "item.rs"

  Scenario: Files at the top of the project form a district named after the project
    Given a code project "shop" with the files "README.md" and "src/catalog/item.rs"
    When the user opens the project
    Then the project shows a district "shop" holding the node "README.md"
    And the project shows the district "src"

  Scenario: Hidden folders, and folders git ignores, are left out
    Given a code project with a hidden folder ".cache" and a folder "build" that git ignores
    When the user opens the project
    Then the project shows nothing for ".cache" or "build"

  Scenario: A folder that is not a git repository leaves out only hidden folders
    Given an added folder that is not a git repository, with a hidden folder ".cache" and a folder "build"
    When the user opens the project
    Then the project shows nothing for ".cache"
    And the project shows the district "build"

  Scenario: A project without files still opens
    Given a code project that contains no files
    When the user opens the project
    Then the project opens and says it has no files

  Scenario: A project set to the longest shared prefix cuts districts below every folder its files share
    Given a code project with the files "src/main/java/acme/billing/Invoice.java" and "src/main/java/acme/orders/Order.java"
    And its district prefix set to "longest-shared-prefix"
    When the user opens the project
    Then the project shows the districts "billing" and "orders"

  Scenario: A project set to no prefix shows its top folders as districts
    Given a code project whose files all live under "src", in the folders "catalog" and "ordering"
    And its district prefix set to "none"
    When the user opens the project
    Then the project shows the district "src"

  Scenario: An unknown district prefix falls back to the default and says so
    Given a code project whose files all live under "src", in the folders "catalog" and "ordering"
    And its district prefix set to "sideways"
    When the user opens the project
    Then the project shows the districts "catalog" and "ordering"
    And the project says the district prefix "sideways" is unknown

  # "Opens the project" is a double-click on the project in the universe, which
  # locks into it and hides the other projects. Enter does the same once
  # STORY-004 lands.
