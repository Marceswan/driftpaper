# Opportunity Cards LWC Design

**Date:** 2025-12-23
**Status:** Approved
**Author:** Claude (via brainstorming session)

## Overview

A Lightning Web Component that displays related Opportunities for an Account in a card-based layout, styled like a trimmed-down highlights panel. Replaces the standard related list with a richer visual experience including product pills, color-coded badges, and flexible display modes.

## Configuration Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `displayMode` | String | `'single'` | `'single'`, `'tabbed'`, or `'multi'` |
| `highlightFields` | String | `'Amount,Probability'` | Comma-separated field API names |
| `recordId` | String | — | Account record context (injected) |

## Display Modes

### Single Mode
- All opportunities in one scrollable list
- Limit: 15 opportunities
- "View All" opens modal with full list

### Tabbed Mode
- Two tabs: "Open" and "Closed" (based on `IsClosed` field)
- Limit: 10 per tab
- Independent sorting per tab
- "View All" navigates to standard Salesforce related list

### Multi Mode
- Two separate card sections stacked vertically
- Limit: 10 per section
- Independent sorting per section
- "View All" opens modal with full list

## Card Layout

```
┌─────────────────────────────────────────────────────────────┐
│  Opportunity Name (link)    [Stage Badge] [Close Date Badge]│
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────┐  ┌─────────────────────┐           │
│  │ Field Label         │  │ Field Label         │           │
│  │ Field Value         │  │ Field Value         │           │
│  └─────────────────────┘  └─────────────────────┘           │
│  (2-column adaptive grid, collapses to 1-column on narrow)  │
├─────────────────────────────────────────────────────────────┤
│  [Product A] [Product B] [Product C] [+4 more]              │
│  (pills wrap to multiple rows, max 10 shown)                │
└─────────────────────────────────────────────────────────────┘
```

### Badge Theming

| Condition | Badge Variant |
|-----------|---------------|
| Stage: Open (IsClosed=false) | `inverse` |
| Stage: Closed Won | `success` |
| Stage: Closed Lost | `warning` |
| Close Date: Future | `inverse` |
| Close Date: Past | `error` |

### Visual Styling
- Cards have light border + subtle shadow (`slds-card` base)
- Small gap (8px) between cards
- Click anywhere on card header navigates to Opportunity
- Product pills use `slds-badge` with hover popover

## Product Pills & Popovers

### Display Rules
- Maximum 10 pills displayed
- Additional products show as "+X more" pill
- Pills wrap naturally to multiple rows
- No products shows muted text: "No products"

### Hover Popover Content
- Product Name
- Quantity
- Unit Price
- Total Price
- Line Description

### Popover Behavior
- Triggered on hover with small delay
- Uses `lightning-popover` or custom tooltip
- Positioned intelligently to avoid viewport clipping
- Dismisses on mouse leave

## Sorting

### Default Sort
Close Date ascending (nearest first)

### Sort Options
- Close Date (Asc/Desc)
- Amount (Asc/Desc)
- Stage (Asc/Desc)
- Name (Asc/Desc)
- Last Modified (Asc/Desc)
- Created Date (Asc/Desc)
- Any field from `highlightFields` configuration

### Independent Sorting
- Tabbed/Multi modes maintain separate sort state per section
- State persists during session (not saved to server)

## Actions

### Header Actions

| Action | Icon | Behavior |
|--------|------|----------|
| New | `utility:add` | Navigate to new Opportunity with `AccountId` pre-filled |
| Sort | `utility:sort` | Dropdown to change sort field/direction |
| Refresh | `utility:refresh` | Manual refresh (supplement to auto-refresh) |

### Per-Card Actions
- Click card header: Navigate to Opportunity record
- Edit icon: Navigate to Opportunity edit page with redirect back

## Data Layer

### Apex Controller: `OpportunityCardsController`

```java
@AuraEnabled(cacheable=true)
public static OpportunityCardsResult getOpportunities(
    Id accountId,
    List<String> fields,
    String sortField,
    String sortDirection,
    Integer limitCount
)
```

### Return Structure

```java
public class OpportunityCardsResult {
    public List<OpportunityWrapper> openOpportunities;
    public List<OpportunityWrapper> closedOpportunities;
    public Integer totalOpenCount;
    public Integer totalClosedCount;
}

public class OpportunityWrapper {
    public Id id;
    public String name;
    public String stageName;
    public Boolean isWon;
    public Boolean isClosed;
    public Date closeDate;
    public Map<String, Object> fields;  // dynamic highlight fields
    public List<LineItemWrapper> lineItems;
}

public class LineItemWrapper {
    public Id id;
    public String productName;
    public Decimal quantity;
    public Decimal unitPrice;
    public Decimal totalPrice;
    public String description;
}
```

### Query Strategy
- Single query for Opportunities with subquery for OpportunityLineItems
- Field set dynamically built from `highlightFields` + required fields
- FLS/CRUD checks via `WITH SECURITY_ENFORCED`

### Real-time Updates
- Use `@wire` with `refreshApex()` capability
- Subscribe to LDS for Opportunity changes on Account
- Refresh triggered when related Opportunities modified

## View All Modal

### Trigger
"View All" link when opportunities exceed display limit

### Modal Features (Single/Multi modes)
- Uses `lightning-modal`
- Header shows count: "All Opportunities (47)"
- Same card rendering as main component
- Sort dropdown in modal header
- Scrollable body with max-height ~70vh
- Close via X button or clicking outside

### Tabbed Mode Behavior
- Does NOT open modal
- Navigates to: `/lightning/r/Account/{recordId}/related/Opportunities/view`

## States & Error Handling

### Loading State
- Spinner centered in component while data loads

### Empty States

| Scenario | Display |
|----------|---------|
| No opportunities at all | "No opportunities" with optional "New Opportunity" button |
| No open opportunities | "No open opportunities" |
| No closed opportunities | "No closed opportunities" |
| No products on opportunity | Muted text: "No products" |

### Error Handling
- Apex errors: icon + message
- Network errors: show retry option
- FLS errors: "You don't have access to view some fields"

### Edge Cases
- Long names: truncate with ellipsis, full text in title
- Currency fields: respect org format via `lightning-formatted-number`
- Date fields: respect user locale via `lightning-formatted-date-time`

## File Structure

```
force-app/main/default/
├── classes/
│   ├── OpportunityCardsController.cls
│   ├── OpportunityCardsController.cls-meta.xml
│   ├── OpportunityCardsControllerTest.cls
│   └── OpportunityCardsControllerTest.cls-meta.xml
└── lwc/
    ├── opportunityCards/
    │   ├── opportunityCards.js
    │   ├── opportunityCards.html
    │   ├── opportunityCards.css
    │   ├── opportunityCards.js-meta.xml
    │   └── __tests__/
    │       └── opportunityCards.test.js
    ├── opportunityCard/
    │   ├── opportunityCard.js
    │   ├── opportunityCard.html
    │   ├── opportunityCard.css
    │   └── opportunityCard.js-meta.xml
    ├── opportunityCardsModal/
    │   ├── opportunityCardsModal.js
    │   ├── opportunityCardsModal.html
    │   └── opportunityCardsModal.js-meta.xml
    └── productPillPopover/
        ├── productPillPopover.js
        ├── productPillPopover.html
        ├── productPillPopover.css
        └── productPillPopover.js-meta.xml
```

## Testing Requirements

### Apex Tests (≥90% coverage)
- With/without Opportunities
- With/without Line Items
- Open vs Closed filtering
- Sorting variations
- Field-level security
- Bulk data (200+ records)

### Jest Tests
- Component rendering in each display mode
- Badge theming logic
- Sort state management
- Navigation calls
- Popover behavior
