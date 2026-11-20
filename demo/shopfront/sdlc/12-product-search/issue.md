# STORY-012: Product search

As a shopper I want to search the catalog so that I find a product quickly.

```gherkin
Feature: Product search

  Scenario: Search by name
    Given the catalog contains "Ceramic Mug"
    When I search for "mug"
    Then "Ceramic Mug" is the first result

  Scenario: No match
    When I search for "zeppelin"
    Then I see "No products match your search."
```
