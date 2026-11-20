# STORY-042: Apply a promo code at checkout (SHOP-42)

As a shopper I want to enter a promo code at checkout so that I pay less.

```gherkin
Feature: Promo code

  Scenario: Valid code
    Given my cart subtotal is 50.00 EUR
    When I apply the code "SAVE10"
    Then I see "Code applied"
    And the total shows a discount of 5.00 EUR
    And the previous total is shown struck through

  Scenario: Invalid code
    When I apply the code "NOPE"
    Then I see "Invalid promo code"
    And the total does not change

  Scenario: Discount cap
    Given my cart subtotal is 1000.00 EUR
    When I apply the code "SAVE50"
    Then the discount is 100.00 EUR

  Scenario: Order records the code
    Given I applied "SAVE10"
    When I place the order
    Then the order records the code "SAVE10" and its discount
```
