# STORY-017: Cart totals

As a shopper I want to see subtotal, shipping and total so that I know what I pay.

```gherkin
Feature: Cart totals

  Scenario: Shipping below the free threshold
    Given my cart holds one "Ceramic Mug"
    Then shipping is 4.90 EUR
    And the total is 19.80 EUR

  Scenario: Free shipping
    Given my cart subtotal is at least 50.00 EUR
    Then shipping is 0.00 EUR
```
