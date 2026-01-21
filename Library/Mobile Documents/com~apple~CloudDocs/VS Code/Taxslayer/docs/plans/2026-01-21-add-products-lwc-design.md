# Add Products to Opportunity - LWC Design

**Date:** 2026-01-21
**Status:** Approved
**Author:** Marc Swan / Whis

## Overview

Custom LWC to replace the standard Salesforce "Add Products" functionality on Opportunities. Key enhancement: read-only Sales Price that auto-calculates based on a user-entered Discount Value.

## Requirements

| Requirement | Detail |
|-------------|--------|
| Usage Context | Opportunity Record Page override + Flow Screen |
| Discount Field | `Discount_Value__c` on OpportunityLineItem (existing) |
| Discount Type | Fixed dollar amount |
| Calculation | Sales Price = List Price - Discount Value |
| Validation | Discount must be ≥ 0 and ≤ List Price |
| Price Book | Use Opportunity's assigned Price Book |

## Architecture

### Component Structure

```
addProductsToOpportunity/        (Main container - orchestrates flow)
├── addProductsSearch/          (Screen 1: Product search & selection)
└── addProductsEdit/            (Screen 2: Edit selected products)
```

### Apex Controller

```apex
AddProductsController.cls
├── getProducts(Id opportunityId, String searchTerm)
├── saveLineItems(Id opportunityId, List<LineItemWrapper> lineItems)
└── getOpportunityPricebook(Id opportunityId)
```

### Data Flow

1. User opens component → Fetch Opportunity's Pricebook
2. Screen 1: Search/select products from PricebookEntry
3. Click "Next" → Pass selected products to Screen 2
4. Screen 2: Edit quantities, discounts → Sales Price auto-calculates
5. Click "Save" → Create OpportunityLineItems via Apex

## Screen 1: Product Selection

### Layout

```
┌─────────────────────────────────────────────────────────┐
│            Add Products                                 │
│        Price Book: [Opportunity's Pricebook Name]       │
├─────────────────────────────────────────────────────────┤
│ [🔍 Search Products...                          ] [⚙]   │
│ Show Selected (3)                                       │
├─────────────────────────────────────────────────────────┤
│ ☐ │ Product Name      │ Code  │ List Price │ Family    │
│───┼───────────────────┼───────┼────────────┼───────────│
│ ☑ │ TaxSlayer Pro     │ TSP01 │ $1,395.00  │ Software  │
│ ☐ │ Annual Seminar    │ AVS01 │ $150.00    │ Services  │
├─────────────────────────────────────────────────────────┤
│                              [Cancel]  [Next]           │
└─────────────────────────────────────────────────────────┘
```

### Columns

| Column | Source Field |
|--------|--------------|
| Product Name | Product2.Name |
| Product Code | Product2.ProductCode |
| List Price | PricebookEntry.UnitPrice |
| Product Description | Product2.Description |
| Product Family | Product2.Family |

### Behavior

- **Search:** Debounced (300ms), searches Product Name and Code
- **Filter button:** Filter by Product Family
- **Show Selected toggle:** Filters to only show checked products
- **Sortable columns:** Click header to sort
- **Checkbox persistence:** Selections maintained across searches
- **Next button:** Disabled until at least 1 product selected

## Screen 2: Edit Selected Products

### Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Edit Selected Products                           │
├─────────────────────────────────────────────────────────────────────┤
│   │ *Product           │ *Qty │ Discount  │ *Sales Price │ Date    │ Description │   │
│───┼────────────────────┼──────┼───────────┼──────────────┼─────────┼─────────────┼───│
│ 1 │ TaxSlayer Pro 🔒   │ [1 ] │ [$100.00] │ $1,295.00    │ [     ] │ [         ] │ 🗑 │
│ 2 │ Annual Seminar 🔒  │ [1 ] │ [$0.00  ] │ $150.00      │ [     ] │ [         ] │ 🗑 │
├─────────────────────────────────────────────────────────────────────┤
│ [Back]                                    [Cancel]  [Save]          │
└─────────────────────────────────────────────────────────────────────┘
```

### Columns

| Column | Type | Editable | Validation |
|--------|------|----------|------------|
| # | Row number | No | Auto-generated |
| Product | Text + lock icon | No | Display only |
| Quantity | Number input | Yes | Required, min 1, default 1 |
| Discount | Currency input | Yes | 0 ≤ value ≤ List Price, default 0 |
| Sales Price | Currency display | No | Reactive: List Price - Discount |
| Date | Date picker | Yes | Optional |
| Description | Text input | Yes | Optional, max 255 chars |
| Delete | Icon button | - | Removes row |

### Reactive Calculation

```javascript
handleDiscountChange(event) {
    const rowIndex = event.target.dataset.index;
    const discount = parseFloat(event.target.value) || 0;
    const listPrice = this.lineItems[rowIndex].listPrice;

    // Validate
    if (discount < 0 || discount > listPrice) {
        // Show inline error
        return;
    }

    // Update Sales Price reactively
    this.lineItems[rowIndex].discountValue = discount;
    this.lineItems[rowIndex].salesPrice = listPrice - discount;
}
```

## Apex Controller Detail

### AddProductsController.cls

```apex
public with sharing class AddProductsController {

    @AuraEnabled(cacheable=true)
    public static PricebookInfo getOpportunityPricebook(Id opportunityId) {
        // Returns Pricebook2Id and Name from Opportunity
    }

    @AuraEnabled(cacheable=true)
    public static List<ProductWrapper> getProducts(
        Id opportunityId,
        String searchTerm,
        String familyFilter
    ) {
        // Query PricebookEntry joined to Product2
        // Filter by Opportunity's Pricebook2Id
        // Return ProductWrapper list
    }

    @AuraEnabled
    public static void saveLineItems(
        Id opportunityId,
        List<LineItemInput> lineItems
    ) {
        // Create OpportunityLineItem records
        // Set UnitPrice = listPrice - discountValue
        // Set Discount_Value__c
    }

    // Wrapper Classes
    public class PricebookInfo {
        @AuraEnabled public Id pricebookId;
        @AuraEnabled public String pricebookName;
    }

    public class ProductWrapper {
        @AuraEnabled public Id pricebookEntryId;
        @AuraEnabled public Id product2Id;
        @AuraEnabled public String productName;
        @AuraEnabled public String productCode;
        @AuraEnabled public Decimal listPrice;
        @AuraEnabled public String description;
        @AuraEnabled public String family;
    }

    public class LineItemInput {
        @AuraEnabled public Id pricebookEntryId;
        @AuraEnabled public Decimal listPrice;
        @AuraEnabled public Integer quantity;
        @AuraEnabled public Decimal discountValue;
        @AuraEnabled public Date serviceDate;
        @AuraEnabled public String description;
    }
}
```

## Error Handling

| Scenario | Handling |
|----------|----------|
| No Pricebook on Opportunity | Show error message, disable Next |
| Search fails | Toast error, keep previous results |
| Save fails | Toast error with details, stay on screen |
| Validation errors | Inline field errors, disable Save |
| No products in Pricebook | Show "No products available" message |

## Test Coverage (90%+)

```apex
AddProductsControllerTest.cls
├── testGetProducts_Success
├── testGetProducts_WithSearchTerm
├── testGetProducts_WithFamilyFilter
├── testGetProducts_NoPricebook (negative)
├── testSaveLineItems_Success
├── testSaveLineItems_WithDiscount
├── testSaveLineItems_BulkInsert (200 items)
├── testGetOpportunityPricebook_Success
└── testGetOpportunityPricebook_NoPricebook
```

## Files to Create

```
force-app/main/default/
├── classes/
│   ├── AddProductsController.cls
│   ├── AddProductsController.cls-meta.xml
│   ├── AddProductsControllerTest.cls
│   └── AddProductsControllerTest.cls-meta.xml
└── lwc/
    ├── addProductsToOpportunity/
    │   ├── addProductsToOpportunity.js
    │   ├── addProductsToOpportunity.html
    │   ├── addProductsToOpportunity.css
    │   └── addProductsToOpportunity.js-meta.xml
    ├── addProductsSearch/
    │   ├── addProductsSearch.js
    │   ├── addProductsSearch.html
    │   ├── addProductsSearch.css
    │   └── addProductsSearch.js-meta.xml
    └── addProductsEdit/
        ├── addProductsEdit.js
        ├── addProductsEdit.html
        ├── addProductsEdit.css
        └── addProductsEdit.js-meta.xml
```

## Flow Compatibility

```javascript
// addProductsToOpportunity.js-meta.xml targets
{
    "targets": {
        "lightning__RecordPage": { "objects": ["Opportunity"] },
        "lightning__FlowScreen": {}
    }
}

// @api properties for Flow
@api recordId;        // Auto-populated on record page
@api opportunityId;   // For flow input
```

## Implementation Notes

1. Use `lightning-datatable` for Screen 1 with row selection
2. Use custom HTML table for Screen 2 (more control over inline editing)
3. Store `listPrice` in JS state but don't display it
4. Calculate `UnitPrice` server-side on save for data integrity
5. Use `refreshApex` after save to update related lists
