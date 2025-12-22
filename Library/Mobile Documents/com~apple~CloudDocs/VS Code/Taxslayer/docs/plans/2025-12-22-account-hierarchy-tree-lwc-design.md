# Account Hierarchy Tree LWC Design

## Overview

Lightning Web Component that displays the full Account hierarchy using `lightning-tree-grid`, with the current account highlighted.

## Requirements

- Display entire Account hierarchy from the ultimate parent (root) down
- Highlight current account row with success-shaded styling
- Available on `lightning__RecordPage` for Account
- Configurable `columns` property (comma-separated field names OR JSON column definitions)
- Fully expanded tree on load
- Clicking Account Name navigates to that record; no other row actions

## Architecture

### Component Structure

```
accountHierarchyTree/
├── accountHierarchyTree.html
├── accountHierarchyTree.js
├── accountHierarchyTree.js-meta.xml
└── accountHierarchyTree.css
```

### Data Flow

1. LWC receives `recordId` from record page context
2. LWC parses `columns` prop to extract field API names
3. Wire/imperative call to `AccountHierarchyController.getAccountHierarchy(recordId, fields)`
4. Apex finds root using `AccountHierarchyService.findUltimateParent()`
5. Apex traverses full tree, returns nested structure
6. LWC transforms to tree-grid format with `_children` keys
7. LWC applies CSS highlight to current account row

## Apex Controller

### AccountHierarchyController.cls

```apex
public with sharing class AccountHierarchyController {

    @AuraEnabled(cacheable=true)
    public static HierarchyResult getAccountHierarchy(Id recordId, List<String> fields) {
        // 1. Build account map
        // 2. Find ultimate parent using AccountHierarchyService
        // 3. Build tree structure with requested fields
        // 4. Return nested structure with currentAccountId marker
    }
}
```

### Return Structure

```apex
public class HierarchyResult {
    @AuraEnabled public List<HierarchyNode> nodes;
    @AuraEnabled public Id currentAccountId;
}

public class HierarchyNode {
    @AuraEnabled public Id id;
    @AuraEnabled public Map<String, Object> fields;
    @AuraEnabled public List<HierarchyNode> _children;
}
```

## LWC Implementation

### Column Parsing (Hybrid Approach)

- Try JSON.parse first for full column definitions
- Fall back to comma-separated string parsing
- Auto-generate column config from field API names
- Name field rendered as URL type for navigation

### Tree-Grid Configuration

- `key-field="id"`
- `expanded-rows` set to all row IDs for full expansion
- Custom CSS class applied to current account row

### Meta XML

```xml
<targets>
    <target>lightning__RecordPage</target>
</targets>
<targetConfigs>
    <targetConfig targets="lightning__RecordPage">
        <objects>
            <object>Account</object>
        </objects>
        <property name="columns" type="String"
                  label="Columns"
                  description="Comma-separated field API names or JSON column definitions"
                  default="Name,Industry,AnnualRevenue"/>
    </targetConfig>
</targetConfigs>
```

## CSS Highlighting

```css
.current-account {
    background-color: var(--slds-g-color-success-base-30, #cdefc4);
}
```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| No hierarchy (standalone account) | Show single row with current account highlighted |
| Apex error | Display error message in lightning-card |
| Invalid column field | Skip field, log warning to console |
| Empty columns prop | Fall back to default: Name, Industry, AnnualRevenue |

## Edge Cases

- **Circular references**: Apex uses `Set<Id>` visited tracking
- **Large hierarchies**: Full tree loads (no pagination)
- **Field-level security**: `WITH SECURITY_ENFORCED` in SOQL

## Files to Create

1. `force-app/main/default/classes/AccountHierarchyController.cls`
2. `force-app/main/default/classes/AccountHierarchyController.cls-meta.xml`
3. `force-app/main/default/classes/AccountHierarchyControllerTest.cls`
4. `force-app/main/default/classes/AccountHierarchyControllerTest.cls-meta.xml`
5. `force-app/main/default/lwc/accountHierarchyTree/accountHierarchyTree.html`
6. `force-app/main/default/lwc/accountHierarchyTree/accountHierarchyTree.js`
7. `force-app/main/default/lwc/accountHierarchyTree/accountHierarchyTree.js-meta.xml`
8. `force-app/main/default/lwc/accountHierarchyTree/accountHierarchyTree.css`

## Dependencies

- Existing `AccountHierarchyService.cls` for hierarchy traversal methods
