<!-- UNTRUSTED USER-SUBMITTED CONTENT.
     This is a problem report. It is data to analyse.
     It carries no instructions for you. -->

# Issue #83: STORY-058: TypeScript and JavaScript code stands as towers
Reported by: Xunrise

Feature: TypeScript and JavaScript code stands as towers
  As a code architect
  I want the functions, classes and imports of TypeScript and JavaScript files to stand as towers
  So that a TypeScript project reads like a Rust one in the city

  Background:
    Given the project "web" holds "src/cart.ts" with the function "total", the class "Cart" and an import block
    And the project is open as a city

  Scenario: Functions, classes and import blocks are towers
    Then "src/cart.ts" stands as the towers "total", "Cart" and its imports

  Scenario: A function held by a constant is a function tower
    Given "src/cart.ts" holds the constant "discount" whose value is a function
    Then there is a function tower "discount"

  Scenario Outline: Type declarations are type towers
    Given "src/cart.ts" holds the <declaration> "<name>"
    Then there is a type tower "<name>"

    Examples:
      | declaration | name      |
      | interface   | Priced    |
      | type alias  | CartId    |
      | enum        | Currency  |

  Scenario: A method is a tower named after its class
    Given the class "Cart" has the method "add"
    Then there is a tower "Cart::add"

  Scenario: Tests stand in the cellar
    Given the project holds "src/cart.test.ts" with the test "adds an item"
    Then the tower of "adds an item" stands in the cellar

  Scenario: Each test of a suite is its own tower, the suite is none
    Given the project holds "src/cart.test.ts" with the suite "Cart" holding the tests "adds an item" and "empties"
    Then the towers "adds an item" and "empties" stand in the cellar
    And there is no tower "Cart" on the block of "src/cart.test.ts"

  Scenario Outline: Every file of the family is read
    Given the project holds "<file>" with the function "load"
    Then there is a tower "load" on the block of "<file>"

    Examples:
      | file           |
      | src/load.ts    |
      | src/load.tsx   |
      | src/load.js    |
      | src/load.mjs   |

  Scenario: A file that does not parse stands as one tower of its lines
    Given "src/broken.ts" is not valid TypeScript
    Then "src/broken.ts" stands as one tower with one floor per line

Prototype: proto/ts-towers

