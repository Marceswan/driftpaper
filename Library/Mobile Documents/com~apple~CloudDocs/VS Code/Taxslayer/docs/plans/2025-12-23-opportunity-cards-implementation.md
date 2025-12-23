# Opportunity Cards LWC Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a card-based related list LWC for displaying Account Opportunities with product pills, badges, and multiple display modes.

**Architecture:** Apex controller returns structured data (open/closed Opportunities with Line Items). Parent LWC (`opportunityCards`) manages display mode and sorting. Child components handle individual cards (`opportunityCard`), product pills (`productPillPopover`), and the View All modal (`opportunityCardsModal`).

**Tech Stack:** Apex, LWC, SLDS, Lightning Modal, NavigationMixin, Wire Service

---

## Phase 1: Apex Controller (TDD)

### Task 1: Create Apex Controller with Wrapper Classes

**Files:**
- Create: `force-app/main/default/classes/OpportunityCardsController.cls`
- Create: `force-app/main/default/classes/OpportunityCardsController.cls-meta.xml`

**Step 1: Create the meta.xml file**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<ApexClass xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>65.0</apiVersion>
    <status>Active</status>
</ApexClass>
```

**Step 2: Create controller with wrapper classes only**

```java
public with sharing class OpportunityCardsController {

    public class OpportunityCardsResult {
        @AuraEnabled public List<OpportunityWrapper> openOpportunities;
        @AuraEnabled public List<OpportunityWrapper> closedOpportunities;
        @AuraEnabled public Integer totalOpenCount;
        @AuraEnabled public Integer totalClosedCount;

        public OpportunityCardsResult() {
            this.openOpportunities = new List<OpportunityWrapper>();
            this.closedOpportunities = new List<OpportunityWrapper>();
            this.totalOpenCount = 0;
            this.totalClosedCount = 0;
        }
    }

    public class OpportunityWrapper {
        @AuraEnabled public Id id;
        @AuraEnabled public String name;
        @AuraEnabled public String stageName;
        @AuraEnabled public Boolean isWon;
        @AuraEnabled public Boolean isClosed;
        @AuraEnabled public Date closeDate;
        @AuraEnabled public Map<String, Object> fields;
        @AuraEnabled public List<LineItemWrapper> lineItems;

        public OpportunityWrapper() {
            this.fields = new Map<String, Object>();
            this.lineItems = new List<LineItemWrapper>();
        }
    }

    public class LineItemWrapper {
        @AuraEnabled public Id id;
        @AuraEnabled public String productName;
        @AuraEnabled public Decimal quantity;
        @AuraEnabled public Decimal unitPrice;
        @AuraEnabled public Decimal totalPrice;
        @AuraEnabled public String description;
    }
}
```

**Step 3: Commit**

```bash
git add force-app/main/default/classes/OpportunityCardsController.cls force-app/main/default/classes/OpportunityCardsController.cls-meta.xml
git commit -m "feat(apex): add OpportunityCardsController with wrapper classes"
```

---

### Task 2: Create Test Class Scaffold

**Files:**
- Create: `force-app/main/default/classes/OpportunityCardsControllerTest.cls`
- Create: `force-app/main/default/classes/OpportunityCardsControllerTest.cls-meta.xml`

**Step 1: Create the meta.xml file**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<ApexClass xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>65.0</apiVersion>
    <status>Active</status>
</ApexClass>
```

**Step 2: Create test class with data factory**

```java
@isTest
private class OpportunityCardsControllerTest {

    @TestSetup
    static void setupTestData() {
        // Create Account
        Account testAccount = new Account(Name = 'Test Account');
        insert testAccount;

        // Create Product
        Product2 testProduct = new Product2(
            Name = 'Test Product',
            ProductCode = 'TEST-001',
            IsActive = true
        );
        insert testProduct;

        // Get standard pricebook
        Id standardPricebookId = Test.getStandardPricebookId();

        // Create PricebookEntry
        PricebookEntry pbe = new PricebookEntry(
            Pricebook2Id = standardPricebookId,
            Product2Id = testProduct.Id,
            UnitPrice = 100.00,
            IsActive = true
        );
        insert pbe;

        // Create Open Opportunities
        List<Opportunity> openOpps = new List<Opportunity>();
        for (Integer i = 0; i < 5; i++) {
            openOpps.add(new Opportunity(
                Name = 'Open Opp ' + i,
                AccountId = testAccount.Id,
                StageName = 'Prospecting',
                CloseDate = Date.today().addDays(30 + i),
                Amount = 1000 * (i + 1)
            ));
        }
        insert openOpps;

        // Create Closed Won Opportunities
        List<Opportunity> closedWonOpps = new List<Opportunity>();
        for (Integer i = 0; i < 3; i++) {
            closedWonOpps.add(new Opportunity(
                Name = 'Closed Won Opp ' + i,
                AccountId = testAccount.Id,
                StageName = 'Closed Won',
                CloseDate = Date.today().addDays(-10 - i),
                Amount = 5000 * (i + 1)
            ));
        }
        insert closedWonOpps;

        // Create Closed Lost Opportunity
        Opportunity closedLostOpp = new Opportunity(
            Name = 'Closed Lost Opp',
            AccountId = testAccount.Id,
            StageName = 'Closed Lost',
            CloseDate = Date.today().addDays(-5),
            Amount = 2000
        );
        insert closedLostOpp;

        // Add Line Items to first open opportunity
        Opportunity oppWithProducts = openOpps[0];
        List<OpportunityLineItem> lineItems = new List<OpportunityLineItem>();
        for (Integer i = 0; i < 3; i++) {
            lineItems.add(new OpportunityLineItem(
                OpportunityId = oppWithProducts.Id,
                PricebookEntryId = pbe.Id,
                Quantity = i + 1,
                UnitPrice = 100.00,
                Description = 'Line item description ' + i
            ));
        }
        insert lineItems;
    }

    static Account getTestAccount() {
        return [SELECT Id FROM Account WHERE Name = 'Test Account' LIMIT 1];
    }

    @isTest
    static void testWrapperClassesInstantiation() {
        // Test that wrapper classes can be instantiated
        OpportunityCardsController.OpportunityCardsResult result = new OpportunityCardsController.OpportunityCardsResult();
        System.assertNotEquals(null, result.openOpportunities, 'openOpportunities should be initialized');
        System.assertNotEquals(null, result.closedOpportunities, 'closedOpportunities should be initialized');
        System.assertEquals(0, result.totalOpenCount, 'totalOpenCount should be 0');
        System.assertEquals(0, result.totalClosedCount, 'totalClosedCount should be 0');

        OpportunityCardsController.OpportunityWrapper oppWrapper = new OpportunityCardsController.OpportunityWrapper();
        System.assertNotEquals(null, oppWrapper.fields, 'fields should be initialized');
        System.assertNotEquals(null, oppWrapper.lineItems, 'lineItems should be initialized');

        OpportunityCardsController.LineItemWrapper lineWrapper = new OpportunityCardsController.LineItemWrapper();
        System.assertNotEquals(null, lineWrapper, 'LineItemWrapper should be instantiated');
    }
}
```

**Step 3: Deploy and run test**

```bash
sf project deploy start --source-dir force-app/main/default/classes/OpportunityCardsController.cls,force-app/main/default/classes/OpportunityCardsControllerTest.cls --target-org <org-alias>
sf apex run test --class-names OpportunityCardsControllerTest --result-format human --synchronous --target-org <org-alias>
```

Expected: 1 test passes

**Step 4: Commit**

```bash
git add force-app/main/default/classes/OpportunityCardsControllerTest.cls force-app/main/default/classes/OpportunityCardsControllerTest.cls-meta.xml
git commit -m "test(apex): add OpportunityCardsControllerTest with test data factory"
```

---

### Task 3: Write Failing Test for getOpportunities

**Files:**
- Modify: `force-app/main/default/classes/OpportunityCardsControllerTest.cls`

**Step 1: Add test for basic getOpportunities**

Add after the existing test method:

```java
    @isTest
    static void testGetOpportunities_ReturnsOpenAndClosed() {
        Account testAccount = getTestAccount();
        List<String> fields = new List<String>{ 'Amount', 'Probability' };

        Test.startTest();
        OpportunityCardsController.OpportunityCardsResult result =
            OpportunityCardsController.getOpportunities(
                testAccount.Id,
                fields,
                'CloseDate',
                'ASC',
                10
            );
        Test.stopTest();

        // We created 5 open and 4 closed (3 won + 1 lost)
        System.assertEquals(5, result.totalOpenCount, 'Should have 5 open opportunities');
        System.assertEquals(4, result.totalClosedCount, 'Should have 4 closed opportunities');
        System.assertEquals(5, result.openOpportunities.size(), 'Should return 5 open opportunities');
        System.assertEquals(4, result.closedOpportunities.size(), 'Should return 4 closed opportunities');
    }
```

**Step 2: Deploy and run test to verify it fails**

```bash
sf project deploy start --source-dir force-app/main/default/classes/OpportunityCardsController.cls,force-app/main/default/classes/OpportunityCardsControllerTest.cls --target-org <org-alias>
sf apex run test --class-names OpportunityCardsControllerTest --result-format human --synchronous --target-org <org-alias>
```

Expected: FAIL - method `getOpportunities` does not exist

**Step 3: Commit the failing test**

```bash
git add force-app/main/default/classes/OpportunityCardsControllerTest.cls
git commit -m "test(apex): add failing test for getOpportunities method"
```

---

### Task 4: Implement getOpportunities Method

**Files:**
- Modify: `force-app/main/default/classes/OpportunityCardsController.cls`

**Step 1: Add the getOpportunities method**

Add after the wrapper classes:

```java
    @AuraEnabled(cacheable=true)
    public static OpportunityCardsResult getOpportunities(
        Id accountId,
        List<String> fields,
        String sortField,
        String sortDirection,
        Integer limitCount
    ) {
        OpportunityCardsResult result = new OpportunityCardsResult();

        if (accountId == null) {
            return result;
        }

        // Build dynamic field list
        Set<String> fieldSet = new Set<String>{
            'Id', 'Name', 'StageName', 'IsWon', 'IsClosed', 'CloseDate'
        };
        if (fields != null) {
            fieldSet.addAll(fields);
        }

        // Validate sort field
        String validSortField = 'CloseDate';
        Set<String> allowedSortFields = new Set<String>{
            'CloseDate', 'Amount', 'StageName', 'Name', 'LastModifiedDate', 'CreatedDate'
        };
        if (fields != null) {
            allowedSortFields.addAll(fields);
        }
        if (String.isNotBlank(sortField) && allowedSortFields.contains(sortField)) {
            validSortField = sortField;
        }

        // Validate sort direction
        String validSortDirection = 'ASC';
        if (String.isNotBlank(sortDirection) &&
            (sortDirection.equalsIgnoreCase('ASC') || sortDirection.equalsIgnoreCase('DESC'))) {
            validSortDirection = sortDirection.toUpperCase();
        }

        // Validate limit
        Integer validLimit = (limitCount != null && limitCount > 0) ? limitCount : 15;

        // Build query
        String fieldList = String.join(new List<String>(fieldSet), ', ');
        String query = 'SELECT ' + fieldList + ', ' +
            '(SELECT Id, Product2.Name, Quantity, UnitPrice, TotalPrice, Description ' +
            'FROM OpportunityLineItems ORDER BY Product2.Name ASC) ' +
            'FROM Opportunity ' +
            'WHERE AccountId = :accountId ' +
            'WITH SECURITY_ENFORCED ' +
            'ORDER BY ' + validSortField + ' ' + validSortDirection + ' NULLS LAST';

        List<Opportunity> allOpps = Database.query(query);

        // Separate open and closed
        List<Opportunity> openOpps = new List<Opportunity>();
        List<Opportunity> closedOpps = new List<Opportunity>();

        for (Opportunity opp : allOpps) {
            if (opp.IsClosed) {
                closedOpps.add(opp);
            } else {
                openOpps.add(opp);
            }
        }

        // Set totals
        result.totalOpenCount = openOpps.size();
        result.totalClosedCount = closedOpps.size();

        // Apply limit and wrap
        Integer openLimit = Math.min(validLimit, openOpps.size());
        Integer closedLimit = Math.min(validLimit, closedOpps.size());

        for (Integer i = 0; i < openLimit; i++) {
            result.openOpportunities.add(wrapOpportunity(openOpps[i], fields));
        }

        for (Integer i = 0; i < closedLimit; i++) {
            result.closedOpportunities.add(wrapOpportunity(closedOpps[i], fields));
        }

        return result;
    }

    private static OpportunityWrapper wrapOpportunity(Opportunity opp, List<String> fields) {
        OpportunityWrapper wrapper = new OpportunityWrapper();
        wrapper.id = opp.Id;
        wrapper.name = opp.Name;
        wrapper.stageName = opp.StageName;
        wrapper.isWon = opp.IsWon;
        wrapper.isClosed = opp.IsClosed;
        wrapper.closeDate = opp.CloseDate;

        // Add dynamic fields
        if (fields != null) {
            for (String field : fields) {
                try {
                    wrapper.fields.put(field, opp.get(field));
                } catch (Exception e) {
                    // Field not accessible, skip
                }
            }
        }

        // Add line items
        if (opp.OpportunityLineItems != null) {
            for (OpportunityLineItem oli : opp.OpportunityLineItems) {
                LineItemWrapper liWrapper = new LineItemWrapper();
                liWrapper.id = oli.Id;
                liWrapper.productName = oli.Product2?.Name;
                liWrapper.quantity = oli.Quantity;
                liWrapper.unitPrice = oli.UnitPrice;
                liWrapper.totalPrice = oli.TotalPrice;
                liWrapper.description = oli.Description;
                wrapper.lineItems.add(liWrapper);
            }
        }

        return wrapper;
    }
```

**Step 2: Deploy and run test to verify it passes**

```bash
sf project deploy start --source-dir force-app/main/default/classes/OpportunityCardsController.cls,force-app/main/default/classes/OpportunityCardsControllerTest.cls --target-org <org-alias>
sf apex run test --class-names OpportunityCardsControllerTest --result-format human --synchronous --target-org <org-alias>
```

Expected: 2 tests pass

**Step 3: Commit**

```bash
git add force-app/main/default/classes/OpportunityCardsController.cls
git commit -m "feat(apex): implement getOpportunities method with dynamic fields"
```

---

### Task 5: Add Tests for Sorting and Line Items

**Files:**
- Modify: `force-app/main/default/classes/OpportunityCardsControllerTest.cls`

**Step 1: Add comprehensive tests**

Add after existing tests:

```java
    @isTest
    static void testGetOpportunities_SortsDescending() {
        Account testAccount = getTestAccount();
        List<String> fields = new List<String>{ 'Amount' };

        Test.startTest();
        OpportunityCardsController.OpportunityCardsResult result =
            OpportunityCardsController.getOpportunities(
                testAccount.Id,
                fields,
                'CloseDate',
                'DESC',
                10
            );
        Test.stopTest();

        // Verify descending order - first open opp should have latest close date
        System.assert(result.openOpportunities.size() > 1, 'Need multiple opps to test sorting');
        Date firstDate = result.openOpportunities[0].closeDate;
        Date secondDate = result.openOpportunities[1].closeDate;
        System.assert(firstDate >= secondDate, 'Should be sorted descending by close date');
    }

    @isTest
    static void testGetOpportunities_IncludesLineItems() {
        Account testAccount = getTestAccount();
        List<String> fields = new List<String>{ 'Amount' };

        Test.startTest();
        OpportunityCardsController.OpportunityCardsResult result =
            OpportunityCardsController.getOpportunities(
                testAccount.Id,
                fields,
                'CloseDate',
                'ASC',
                10
            );
        Test.stopTest();

        // Find the opportunity with line items (Open Opp 0)
        Boolean foundOppWithLineItems = false;
        for (OpportunityCardsController.OpportunityWrapper opp : result.openOpportunities) {
            if (opp.name == 'Open Opp 0') {
                foundOppWithLineItems = true;
                System.assertEquals(3, opp.lineItems.size(), 'Should have 3 line items');

                // Verify line item data
                OpportunityCardsController.LineItemWrapper firstItem = opp.lineItems[0];
                System.assertNotEquals(null, firstItem.productName, 'Product name should be set');
                System.assertNotEquals(null, firstItem.quantity, 'Quantity should be set');
                System.assertNotEquals(null, firstItem.unitPrice, 'Unit price should be set');
                System.assertNotEquals(null, firstItem.totalPrice, 'Total price should be set');
                break;
            }
        }
        System.assert(foundOppWithLineItems, 'Should find opportunity with line items');
    }

    @isTest
    static void testGetOpportunities_RespectsLimit() {
        Account testAccount = getTestAccount();
        List<String> fields = new List<String>();

        Test.startTest();
        OpportunityCardsController.OpportunityCardsResult result =
            OpportunityCardsController.getOpportunities(
                testAccount.Id,
                fields,
                'CloseDate',
                'ASC',
                2  // Only return 2
            );
        Test.stopTest();

        // Should respect limit but still have correct totals
        System.assertEquals(5, result.totalOpenCount, 'Total open count should be 5');
        System.assertEquals(2, result.openOpportunities.size(), 'Should only return 2 open opportunities');
    }

    @isTest
    static void testGetOpportunities_IncludesDynamicFields() {
        Account testAccount = getTestAccount();
        List<String> fields = new List<String>{ 'Amount', 'Probability' };

        Test.startTest();
        OpportunityCardsController.OpportunityCardsResult result =
            OpportunityCardsController.getOpportunities(
                testAccount.Id,
                fields,
                'CloseDate',
                'ASC',
                10
            );
        Test.stopTest();

        // Check dynamic fields are included
        OpportunityCardsController.OpportunityWrapper firstOpp = result.openOpportunities[0];
        System.assert(firstOpp.fields.containsKey('Amount'), 'Should include Amount field');
        System.assertNotEquals(null, firstOpp.fields.get('Amount'), 'Amount should have value');
    }

    @isTest
    static void testGetOpportunities_NullAccountId() {
        List<String> fields = new List<String>();

        Test.startTest();
        OpportunityCardsController.OpportunityCardsResult result =
            OpportunityCardsController.getOpportunities(
                null,
                fields,
                'CloseDate',
                'ASC',
                10
            );
        Test.stopTest();

        System.assertEquals(0, result.totalOpenCount, 'Should return empty result for null account');
        System.assertEquals(0, result.openOpportunities.size(), 'Should have no opportunities');
    }

    @isTest
    static void testGetOpportunities_InvalidSortField() {
        Account testAccount = getTestAccount();
        List<String> fields = new List<String>();

        Test.startTest();
        // Should not throw error, should default to CloseDate
        OpportunityCardsController.OpportunityCardsResult result =
            OpportunityCardsController.getOpportunities(
                testAccount.Id,
                fields,
                'InvalidField__c',  // Invalid field
                'ASC',
                10
            );
        Test.stopTest();

        System.assertNotEquals(null, result, 'Should return result even with invalid sort field');
        System.assert(result.openOpportunities.size() > 0, 'Should still return opportunities');
    }

    @isTest
    static void testGetOpportunities_ClosedWonVsLost() {
        Account testAccount = getTestAccount();
        List<String> fields = new List<String>();

        Test.startTest();
        OpportunityCardsController.OpportunityCardsResult result =
            OpportunityCardsController.getOpportunities(
                testAccount.Id,
                fields,
                'CloseDate',
                'ASC',
                10
            );
        Test.stopTest();

        // Verify isWon flag is correctly set
        Integer wonCount = 0;
        Integer lostCount = 0;
        for (OpportunityCardsController.OpportunityWrapper opp : result.closedOpportunities) {
            if (opp.isWon) {
                wonCount++;
            } else {
                lostCount++;
            }
        }
        System.assertEquals(3, wonCount, 'Should have 3 closed won opportunities');
        System.assertEquals(1, lostCount, 'Should have 1 closed lost opportunity');
    }

    @isTest
    static void testGetOpportunities_BulkData() {
        Account testAccount = getTestAccount();

        // Create 200 more opportunities
        List<Opportunity> bulkOpps = new List<Opportunity>();
        for (Integer i = 0; i < 200; i++) {
            bulkOpps.add(new Opportunity(
                Name = 'Bulk Opp ' + i,
                AccountId = testAccount.Id,
                StageName = 'Prospecting',
                CloseDate = Date.today().addDays(i),
                Amount = 100
            ));
        }
        insert bulkOpps;

        List<String> fields = new List<String>{ 'Amount' };

        Test.startTest();
        OpportunityCardsController.OpportunityCardsResult result =
            OpportunityCardsController.getOpportunities(
                testAccount.Id,
                fields,
                'CloseDate',
                'ASC',
                15
            );
        Test.stopTest();

        // Should handle bulk data and respect limit
        System.assertEquals(205, result.totalOpenCount, 'Total should include all 205 open opps');
        System.assertEquals(15, result.openOpportunities.size(), 'Should only return 15');
    }
```

**Step 2: Deploy and run all tests**

```bash
sf project deploy start --source-dir force-app/main/default/classes/OpportunityCardsController.cls,force-app/main/default/classes/OpportunityCardsControllerTest.cls --target-org <org-alias>
sf apex run test --class-names OpportunityCardsControllerTest --result-format human --synchronous --code-coverage --target-org <org-alias>
```

Expected: All tests pass, coverage ≥90%

**Step 3: Commit**

```bash
git add force-app/main/default/classes/OpportunityCardsControllerTest.cls
git commit -m "test(apex): add comprehensive tests for sorting, line items, and bulk data"
```

---

## Phase 2: productPillPopover LWC

### Task 6: Create productPillPopover Component

**Files:**
- Create: `force-app/main/default/lwc/productPillPopover/productPillPopover.js`
- Create: `force-app/main/default/lwc/productPillPopover/productPillPopover.html`
- Create: `force-app/main/default/lwc/productPillPopover/productPillPopover.css`
- Create: `force-app/main/default/lwc/productPillPopover/productPillPopover.js-meta.xml`

**Step 1: Create meta.xml**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LightningComponentBundle xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>65.0</apiVersion>
    <isExposed>false</isExposed>
    <description>Product pill with hover popover showing line item details</description>
</LightningComponentBundle>
```

**Step 2: Create HTML template**

```html
<template>
    <div class="pill-container"
         onmouseenter={handleMouseEnter}
         onmouseleave={handleMouseLeave}>
        <span class="slds-badge slds-badge_lightest product-pill" title={productName}>
            {displayName}
        </span>

        <template if:true={showPopover}>
            <section class="slds-popover slds-popover_tooltip slds-nubbin_bottom-left popover-container"
                     role="tooltip">
                <div class="slds-popover__body">
                    <dl class="slds-dl_horizontal slds-text-body_small">
                        <dt class="slds-dl_horizontal__label">
                            <span class="slds-truncate">Product</span>
                        </dt>
                        <dd class="slds-dl_horizontal__detail">
                            <span class="slds-text-title_bold">{productName}</span>
                        </dd>

                        <dt class="slds-dl_horizontal__label">
                            <span class="slds-truncate">Quantity</span>
                        </dt>
                        <dd class="slds-dl_horizontal__detail">
                            <lightning-formatted-number value={quantity}
                                                        maximum-fraction-digits="2">
                            </lightning-formatted-number>
                        </dd>

                        <dt class="slds-dl_horizontal__label">
                            <span class="slds-truncate">Unit Price</span>
                        </dt>
                        <dd class="slds-dl_horizontal__detail">
                            <lightning-formatted-number value={unitPrice}
                                                        format-style="currency"
                                                        currency-code="USD">
                            </lightning-formatted-number>
                        </dd>

                        <dt class="slds-dl_horizontal__label">
                            <span class="slds-truncate">Total Price</span>
                        </dt>
                        <dd class="slds-dl_horizontal__detail">
                            <lightning-formatted-number value={totalPrice}
                                                        format-style="currency"
                                                        currency-code="USD">
                            </lightning-formatted-number>
                        </dd>

                        <template if:true={description}>
                            <dt class="slds-dl_horizontal__label">
                                <span class="slds-truncate">Description</span>
                            </dt>
                            <dd class="slds-dl_horizontal__detail description-text">
                                {description}
                            </dd>
                        </template>
                    </dl>
                </div>
            </section>
        </template>
    </div>
</template>
```

**Step 3: Create JavaScript**

```javascript
import { LightningElement, api } from 'lwc';

const MAX_NAME_LENGTH = 20;
const HOVER_DELAY_MS = 200;

export default class ProductPillPopover extends LightningElement {
    @api productName;
    @api quantity;
    @api unitPrice;
    @api totalPrice;
    @api description;

    showPopover = false;
    hoverTimeout;

    get displayName() {
        if (!this.productName) return '';
        if (this.productName.length <= MAX_NAME_LENGTH) {
            return this.productName;
        }
        return this.productName.substring(0, MAX_NAME_LENGTH - 1) + '…';
    }

    handleMouseEnter() {
        this.hoverTimeout = setTimeout(() => {
            this.showPopover = true;
        }, HOVER_DELAY_MS);
    }

    handleMouseLeave() {
        if (this.hoverTimeout) {
            clearTimeout(this.hoverTimeout);
        }
        this.showPopover = false;
    }

    disconnectedCallback() {
        if (this.hoverTimeout) {
            clearTimeout(this.hoverTimeout);
        }
    }
}
```

**Step 4: Create CSS**

```css
.pill-container {
    display: inline-block;
    position: relative;
    margin: 0.125rem;
}

.product-pill {
    cursor: default;
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.popover-container {
    position: absolute;
    bottom: 100%;
    left: 0;
    margin-bottom: 0.5rem;
    z-index: 9001;
    min-width: 200px;
    max-width: 300px;
}

.slds-dl_horizontal__label {
    width: 80px;
}

.slds-dl_horizontal__detail {
    padding-left: 0.5rem;
}

.description-text {
    white-space: normal;
    word-wrap: break-word;
}
```

**Step 5: Commit**

```bash
git add force-app/main/default/lwc/productPillPopover
git commit -m "feat(lwc): add productPillPopover component with hover tooltip"
```

---

## Phase 3: opportunityCard LWC

### Task 7: Create opportunityCard Component

**Files:**
- Create: `force-app/main/default/lwc/opportunityCard/opportunityCard.js`
- Create: `force-app/main/default/lwc/opportunityCard/opportunityCard.html`
- Create: `force-app/main/default/lwc/opportunityCard/opportunityCard.css`
- Create: `force-app/main/default/lwc/opportunityCard/opportunityCard.js-meta.xml`

**Step 1: Create meta.xml**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LightningComponentBundle xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>65.0</apiVersion>
    <isExposed>false</isExposed>
    <description>Individual opportunity card with badges and product pills</description>
</LightningComponentBundle>
```

**Step 2: Create HTML template**

```html
<template>
    <article class="slds-card opportunity-card">
        <!-- Header with name and badges -->
        <div class="slds-card__header slds-grid">
            <header class="slds-media slds-media_center slds-has-flexi-truncate">
                <div class="slds-media__body">
                    <h2 class="slds-card__header-title">
                        <a href={opportunityUrl}
                           class="slds-card__header-link slds-truncate"
                           title={opportunity.name}
                           onclick={handleNavigate}>
                            {opportunity.name}
                        </a>
                    </h2>
                </div>
                <div class="slds-no-flex badges-container">
                    <span class={stageBadgeClass}>{opportunity.stageName}</span>
                    <span class={closeDateBadgeClass}>{formattedCloseDate}</span>
                </div>
            </header>
            <div class="slds-no-flex">
                <lightning-button-icon
                    icon-name="utility:edit"
                    alternative-text="Edit"
                    title="Edit Opportunity"
                    onclick={handleEdit}
                    class="slds-m-left_x-small">
                </lightning-button-icon>
            </div>
        </div>

        <!-- Highlight fields -->
        <div class="slds-card__body slds-card__body_inner">
            <div class="slds-grid slds-wrap highlight-fields">
                <template for:each={highlightFieldsList} for:item="field">
                    <div key={field.apiName} class="slds-col slds-size_1-of-1 slds-medium-size_1-of-2 field-item">
                        <span class="slds-text-title_caps field-label">{field.label}</span>
                        <div class="field-value">
                            <template if:true={field.isCurrency}>
                                <lightning-formatted-number
                                    value={field.value}
                                    format-style="currency"
                                    currency-code="USD">
                                </lightning-formatted-number>
                            </template>
                            <template if:true={field.isPercent}>
                                <lightning-formatted-number
                                    value={field.percentValue}
                                    format-style="percent"
                                    maximum-fraction-digits="0">
                                </lightning-formatted-number>
                            </template>
                            <template if:true={field.isDate}>
                                <lightning-formatted-date-time
                                    value={field.value}>
                                </lightning-formatted-date-time>
                            </template>
                            <template if:true={field.isText}>
                                {field.value}
                            </template>
                        </div>
                    </div>
                </template>
            </div>

            <!-- Product pills -->
            <div class="slds-m-top_small products-section">
                <template if:true={hasProducts}>
                    <div class="pills-wrapper">
                        <template for:each={displayedLineItems} for:item="item">
                            <c-product-pill-popover
                                key={item.id}
                                product-name={item.productName}
                                quantity={item.quantity}
                                unit-price={item.unitPrice}
                                total-price={item.totalPrice}
                                description={item.description}>
                            </c-product-pill-popover>
                        </template>
                        <template if:true={hasMoreProducts}>
                            <span class="slds-badge slds-badge_lightest more-pill"
                                  title={moreProductsTooltip}
                                  onclick={handleShowMoreProducts}>
                                +{remainingProductCount} more
                            </span>
                        </template>
                    </div>
                </template>
                <template if:false={hasProducts}>
                    <span class="slds-text-color_weak slds-text-body_small">No products</span>
                </template>
            </div>
        </div>
    </article>
</template>
```

**Step 3: Create JavaScript**

```javascript
import { LightningElement, api } from 'lwc';
import { NavigationMixin } from 'lightning/navigation';

const MAX_DISPLAYED_PRODUCTS = 10;
const CURRENCY_FIELDS = ['Amount', 'ExpectedRevenue'];
const PERCENT_FIELDS = ['Probability'];
const DATE_FIELDS = ['CloseDate', 'CreatedDate', 'LastModifiedDate'];

export default class OpportunityCard extends NavigationMixin(LightningElement) {
    @api opportunity;
    @api highlightFields = '';
    @api fieldLabels = {};

    get opportunityUrl() {
        return '/' + this.opportunity?.id;
    }

    get formattedCloseDate() {
        if (!this.opportunity?.closeDate) return '';
        const date = new Date(this.opportunity.closeDate);
        return date.toLocaleDateString();
    }

    get isPastDue() {
        if (!this.opportunity?.closeDate) return false;
        const closeDate = new Date(this.opportunity.closeDate);
        const today = new Date();
        today.setHours(0, 0, 0, 0);
        return closeDate < today;
    }

    get stageBadgeClass() {
        let baseClass = 'slds-badge slds-m-left_x-small';
        if (!this.opportunity?.isClosed) {
            return baseClass + ' slds-badge_inverse';
        }
        if (this.opportunity.isWon) {
            return baseClass + ' slds-theme_success';
        }
        return baseClass + ' slds-theme_warning';
    }

    get closeDateBadgeClass() {
        let baseClass = 'slds-badge slds-m-left_x-small';
        if (this.isPastDue) {
            return baseClass + ' slds-theme_error';
        }
        return baseClass + ' slds-badge_inverse';
    }

    get highlightFieldsList() {
        if (!this.highlightFields || !this.opportunity) return [];

        const fields = this.highlightFields.split(',').map(f => f.trim()).filter(Boolean);
        return fields.map(apiName => {
            const value = this.opportunity.fields?.[apiName];
            const label = this.fieldLabels[apiName] || this.formatFieldLabel(apiName);

            return {
                apiName,
                label,
                value,
                isCurrency: CURRENCY_FIELDS.includes(apiName),
                isPercent: PERCENT_FIELDS.includes(apiName),
                isDate: DATE_FIELDS.includes(apiName),
                isText: !CURRENCY_FIELDS.includes(apiName) &&
                        !PERCENT_FIELDS.includes(apiName) &&
                        !DATE_FIELDS.includes(apiName),
                percentValue: PERCENT_FIELDS.includes(apiName) ? (value / 100) : null
            };
        });
    }

    get hasProducts() {
        return this.opportunity?.lineItems?.length > 0;
    }

    get displayedLineItems() {
        if (!this.opportunity?.lineItems) return [];
        return this.opportunity.lineItems.slice(0, MAX_DISPLAYED_PRODUCTS);
    }

    get hasMoreProducts() {
        return this.opportunity?.lineItems?.length > MAX_DISPLAYED_PRODUCTS;
    }

    get remainingProductCount() {
        if (!this.opportunity?.lineItems) return 0;
        return this.opportunity.lineItems.length - MAX_DISPLAYED_PRODUCTS;
    }

    get moreProductsTooltip() {
        return `${this.remainingProductCount} more products`;
    }

    formatFieldLabel(apiName) {
        return apiName
            .replace(/([A-Z])/g, ' $1')
            .replace(/^./, str => str.toUpperCase())
            .trim();
    }

    handleNavigate(event) {
        event.preventDefault();
        this[NavigationMixin.Navigate]({
            type: 'standard__recordPage',
            attributes: {
                recordId: this.opportunity.id,
                actionName: 'view'
            }
        });
    }

    handleEdit(event) {
        event.stopPropagation();
        this[NavigationMixin.Navigate]({
            type: 'standard__recordPage',
            attributes: {
                recordId: this.opportunity.id,
                actionName: 'edit'
            }
        });
    }

    handleShowMoreProducts() {
        // Dispatch event for parent to handle (could open modal with all products)
        this.dispatchEvent(new CustomEvent('showmoreproducts', {
            detail: {
                opportunityId: this.opportunity.id,
                lineItems: this.opportunity.lineItems
            }
        }));
    }
}
```

**Step 4: Create CSS**

```css
.opportunity-card {
    border: 1px solid #d8dde6;
    border-radius: 0.25rem;
    box-shadow: 0 2px 2px 0 rgba(0, 0, 0, 0.1);
    margin-bottom: 0.5rem;
}

.badges-container {
    display: flex;
    align-items: center;
}

.highlight-fields {
    padding: 0.5rem 0;
}

.field-item {
    padding: 0.25rem 0.5rem;
}

.field-label {
    color: #706e6b;
    font-size: 0.75rem;
    display: block;
    margin-bottom: 0.125rem;
}

.field-value {
    font-size: 0.875rem;
    font-weight: 500;
}

.products-section {
    border-top: 1px solid #e5e5e5;
    padding-top: 0.5rem;
}

.pills-wrapper {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
}

.more-pill {
    cursor: pointer;
}

.more-pill:hover {
    background-color: #d8dde6;
}
```

**Step 5: Commit**

```bash
git add force-app/main/default/lwc/opportunityCard
git commit -m "feat(lwc): add opportunityCard component with badges and pills"
```

---

## Phase 4: opportunityCards Main LWC

### Task 8: Create opportunityCards Container Component

**Files:**
- Create: `force-app/main/default/lwc/opportunityCards/opportunityCards.js`
- Create: `force-app/main/default/lwc/opportunityCards/opportunityCards.html`
- Create: `force-app/main/default/lwc/opportunityCards/opportunityCards.css`
- Create: `force-app/main/default/lwc/opportunityCards/opportunityCards.js-meta.xml`

**Step 1: Create meta.xml**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LightningComponentBundle xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>65.0</apiVersion>
    <isExposed>true</isExposed>
    <masterLabel>Opportunity Cards</masterLabel>
    <description>Card-based display of related Opportunities with product pills and badges</description>
    <targets>
        <target>lightning__RecordPage</target>
        <target>lightning__AppPage</target>
    </targets>
    <targetConfigs>
        <targetConfig targets="lightning__RecordPage">
            <objects>
                <object>Account</object>
            </objects>
            <property name="displayMode" type="String" default="single"
                      label="Display Mode"
                      description="single = all in one list, tabbed = open/closed tabs, multi = separate sections"
                      datasource="single,tabbed,multi"/>
            <property name="highlightFields" type="String" default="Amount,Probability"
                      label="Highlight Fields"
                      description="Comma-separated API names of fields to display"/>
        </targetConfig>
        <targetConfig targets="lightning__AppPage">
            <property name="displayMode" type="String" default="single"
                      label="Display Mode"
                      datasource="single,tabbed,multi"/>
            <property name="highlightFields" type="String" default="Amount,Probability"
                      label="Highlight Fields"/>
        </targetConfig>
    </targetConfigs>
</LightningComponentBundle>
```

**Step 2: Create HTML template**

```html
<template>
    <!-- SINGLE MODE -->
    <template if:true={isSingleMode}>
        <lightning-card title="Opportunities" icon-name="standard:opportunity">
            <div slot="actions">
                <lightning-button-menu
                    alternative-text="Sort options"
                    icon-name="utility:sort"
                    onselect={handleSortChange}
                    menu-alignment="right"
                    class="slds-m-right_x-small">
                    <template for:each={sortOptions} for:item="option">
                        <lightning-menu-item
                            key={option.value}
                            value={option.value}
                            label={option.label}
                            checked={option.checked}>
                        </lightning-menu-item>
                    </template>
                </lightning-button-menu>
                <lightning-button-icon
                    icon-name="utility:refresh"
                    alternative-text="Refresh"
                    onclick={handleRefresh}
                    class="slds-m-right_x-small">
                </lightning-button-icon>
                <lightning-button-icon
                    icon-name="utility:add"
                    alternative-text="New Opportunity"
                    onclick={handleNew}>
                </lightning-button-icon>
            </div>

            <div class="slds-card__body slds-card__body_inner">
                <template if:true={isLoading}>
                    <lightning-spinner alternative-text="Loading" size="medium"></lightning-spinner>
                </template>

                <template if:true={hasError}>
                    <div class="slds-text-color_error slds-p-around_medium">
                        <lightning-icon icon-name="utility:error" size="small" class="slds-m-right_x-small"></lightning-icon>
                        {errorMessage}
                    </div>
                </template>

                <template if:false={isLoading}>
                    <template if:false={hasError}>
                        <template if:true={hasOpportunities}>
                            <div class="cards-container">
                                <template for:each={allOpportunities} for:item="opp">
                                    <c-opportunity-card
                                        key={opp.id}
                                        opportunity={opp}
                                        highlight-fields={highlightFields}
                                        field-labels={fieldLabels}>
                                    </c-opportunity-card>
                                </template>
                            </div>
                            <template if:true={showViewAllSingle}>
                                <div class="slds-text-align_center slds-m-top_small">
                                    <lightning-button
                                        label={viewAllLabel}
                                        onclick={handleViewAll}
                                        variant="base">
                                    </lightning-button>
                                </div>
                            </template>
                        </template>
                        <template if:false={hasOpportunities}>
                            <div class="slds-text-align_center slds-p-around_medium slds-text-color_weak">
                                <p>No opportunities</p>
                                <lightning-button
                                    label="New Opportunity"
                                    onclick={handleNew}
                                    variant="neutral"
                                    class="slds-m-top_small">
                                </lightning-button>
                            </div>
                        </template>
                    </template>
                </template>
            </div>
        </lightning-card>
    </template>

    <!-- TABBED MODE -->
    <template if:true={isTabbedMode}>
        <lightning-card title="Opportunities" icon-name="standard:opportunity">
            <div slot="actions">
                <lightning-button-icon
                    icon-name="utility:refresh"
                    alternative-text="Refresh"
                    onclick={handleRefresh}
                    class="slds-m-right_x-small">
                </lightning-button-icon>
                <lightning-button-icon
                    icon-name="utility:add"
                    alternative-text="New Opportunity"
                    onclick={handleNew}>
                </lightning-button-icon>
            </div>

            <template if:true={isLoading}>
                <div class="slds-p-around_large">
                    <lightning-spinner alternative-text="Loading" size="medium"></lightning-spinner>
                </div>
            </template>

            <template if:false={isLoading}>
                <lightning-tabset>
                    <lightning-tab label={openTabLabel}>
                        <div class="tab-actions slds-p-horizontal_medium slds-p-bottom_x-small">
                            <lightning-button-menu
                                alternative-text="Sort options"
                                icon-name="utility:sort"
                                onselect={handleOpenSortChange}
                                menu-alignment="right">
                                <template for:each={openSortOptions} for:item="option">
                                    <lightning-menu-item
                                        key={option.value}
                                        value={option.value}
                                        label={option.label}
                                        checked={option.checked}>
                                    </lightning-menu-item>
                                </template>
                            </lightning-button-menu>
                        </div>
                        <div class="slds-p-horizontal_medium">
                            <template if:true={hasOpenOpportunities}>
                                <template for:each={displayedOpenOpportunities} for:item="opp">
                                    <c-opportunity-card
                                        key={opp.id}
                                        opportunity={opp}
                                        highlight-fields={highlightFields}
                                        field-labels={fieldLabels}>
                                    </c-opportunity-card>
                                </template>
                                <template if:true={showViewAllOpen}>
                                    <div class="slds-text-align_center slds-m-top_small">
                                        <lightning-button
                                            label={viewAllOpenLabel}
                                            onclick={handleViewAllOpen}
                                            variant="base">
                                        </lightning-button>
                                    </div>
                                </template>
                            </template>
                            <template if:false={hasOpenOpportunities}>
                                <p class="slds-text-color_weak slds-text-align_center slds-p-around_medium">
                                    No open opportunities
                                </p>
                            </template>
                        </div>
                    </lightning-tab>
                    <lightning-tab label={closedTabLabel}>
                        <div class="tab-actions slds-p-horizontal_medium slds-p-bottom_x-small">
                            <lightning-button-menu
                                alternative-text="Sort options"
                                icon-name="utility:sort"
                                onselect={handleClosedSortChange}
                                menu-alignment="right">
                                <template for:each={closedSortOptions} for:item="option">
                                    <lightning-menu-item
                                        key={option.value}
                                        value={option.value}
                                        label={option.label}
                                        checked={option.checked}>
                                    </lightning-menu-item>
                                </template>
                            </lightning-button-menu>
                        </div>
                        <div class="slds-p-horizontal_medium">
                            <template if:true={hasClosedOpportunities}>
                                <template for:each={displayedClosedOpportunities} for:item="opp">
                                    <c-opportunity-card
                                        key={opp.id}
                                        opportunity={opp}
                                        highlight-fields={highlightFields}
                                        field-labels={fieldLabels}>
                                    </c-opportunity-card>
                                </template>
                                <template if:true={showViewAllClosed}>
                                    <div class="slds-text-align_center slds-m-top_small">
                                        <lightning-button
                                            label={viewAllClosedLabel}
                                            onclick={handleViewAllClosed}
                                            variant="base">
                                        </lightning-button>
                                    </div>
                                </template>
                            </template>
                            <template if:false={hasClosedOpportunities}>
                                <p class="slds-text-color_weak slds-text-align_center slds-p-around_medium">
                                    No closed opportunities
                                </p>
                            </template>
                        </div>
                    </lightning-tab>
                </lightning-tabset>
            </template>
        </lightning-card>
    </template>

    <!-- MULTI MODE -->
    <template if:true={isMultiMode}>
        <!-- Open Opportunities Section -->
        <lightning-card title="Open Opportunities" icon-name="standard:opportunity" class="slds-m-bottom_medium">
            <div slot="actions">
                <lightning-button-menu
                    alternative-text="Sort options"
                    icon-name="utility:sort"
                    onselect={handleOpenSortChange}
                    menu-alignment="right"
                    class="slds-m-right_x-small">
                    <template for:each={openSortOptions} for:item="option">
                        <lightning-menu-item
                            key={option.value}
                            value={option.value}
                            label={option.label}
                            checked={option.checked}>
                        </lightning-menu-item>
                    </template>
                </lightning-button-menu>
                <lightning-button-icon
                    icon-name="utility:refresh"
                    alternative-text="Refresh"
                    onclick={handleRefresh}
                    class="slds-m-right_x-small">
                </lightning-button-icon>
                <lightning-button-icon
                    icon-name="utility:add"
                    alternative-text="New Opportunity"
                    onclick={handleNew}>
                </lightning-button-icon>
            </div>
            <div class="slds-card__body slds-card__body_inner">
                <template if:true={isLoading}>
                    <lightning-spinner alternative-text="Loading" size="medium"></lightning-spinner>
                </template>
                <template if:false={isLoading}>
                    <template if:true={hasOpenOpportunities}>
                        <template for:each={displayedOpenOpportunities} for:item="opp">
                            <c-opportunity-card
                                key={opp.id}
                                opportunity={opp}
                                highlight-fields={highlightFields}
                                field-labels={fieldLabels}>
                            </c-opportunity-card>
                        </template>
                        <template if:true={showViewAllOpen}>
                            <div class="slds-text-align_center slds-m-top_small">
                                <lightning-button
                                    label={viewAllOpenLabel}
                                    onclick={handleViewAllOpenModal}
                                    variant="base">
                                </lightning-button>
                            </div>
                        </template>
                    </template>
                    <template if:false={hasOpenOpportunities}>
                        <p class="slds-text-color_weak slds-text-align_center slds-p-around_medium">
                            No open opportunities
                        </p>
                    </template>
                </template>
            </div>
        </lightning-card>

        <!-- Closed Opportunities Section -->
        <lightning-card title="Closed Opportunities" icon-name="standard:opportunity">
            <div slot="actions">
                <lightning-button-menu
                    alternative-text="Sort options"
                    icon-name="utility:sort"
                    onselect={handleClosedSortChange}
                    menu-alignment="right"
                    class="slds-m-right_x-small">
                    <template for:each={closedSortOptions} for:item="option">
                        <lightning-menu-item
                            key={option.value}
                            value={option.value}
                            label={option.label}
                            checked={option.checked}>
                        </lightning-menu-item>
                    </template>
                </lightning-button-menu>
                <lightning-button-icon
                    icon-name="utility:refresh"
                    alternative-text="Refresh"
                    onclick={handleRefresh}>
                </lightning-button-icon>
            </div>
            <div class="slds-card__body slds-card__body_inner">
                <template if:false={isLoading}>
                    <template if:true={hasClosedOpportunities}>
                        <template for:each={displayedClosedOpportunities} for:item="opp">
                            <c-opportunity-card
                                key={opp.id}
                                opportunity={opp}
                                highlight-fields={highlightFields}
                                field-labels={fieldLabels}>
                            </c-opportunity-card>
                        </template>
                        <template if:true={showViewAllClosed}>
                            <div class="slds-text-align_center slds-m-top_small">
                                <lightning-button
                                    label={viewAllClosedLabel}
                                    onclick={handleViewAllClosedModal}
                                    variant="base">
                                </lightning-button>
                            </div>
                        </template>
                    </template>
                    <template if:false={hasClosedOpportunities}>
                        <p class="slds-text-color_weak slds-text-align_center slds-p-around_medium">
                            No closed opportunities
                        </p>
                    </template>
                </template>
            </div>
        </lightning-card>
    </template>
</template>
```

**Step 3: Create JavaScript**

```javascript
import { LightningElement, api, wire } from 'lwc';
import { NavigationMixin } from 'lightning/navigation';
import { refreshApex } from '@salesforce/apex';
import { getObjectInfo } from 'lightning/uiObjectInfoApi';
import OPPORTUNITY_OBJECT from '@salesforce/schema/Opportunity';
import getOpportunities from '@salesforce/apex/OpportunityCardsController.getOpportunities';
import OpportunityCardsModal from 'c/opportunityCardsModal';

const LIMIT_SINGLE = 15;
const LIMIT_TABBED_MULTI = 10;

const BASE_SORT_OPTIONS = [
    { value: 'CloseDate-ASC', label: 'Close Date (Earliest First)', field: 'CloseDate', direction: 'ASC' },
    { value: 'CloseDate-DESC', label: 'Close Date (Latest First)', field: 'CloseDate', direction: 'DESC' },
    { value: 'Amount-DESC', label: 'Amount (Highest First)', field: 'Amount', direction: 'DESC' },
    { value: 'Amount-ASC', label: 'Amount (Lowest First)', field: 'Amount', direction: 'ASC' },
    { value: 'Name-ASC', label: 'Name (A-Z)', field: 'Name', direction: 'ASC' },
    { value: 'Name-DESC', label: 'Name (Z-A)', field: 'Name', direction: 'DESC' },
    { value: 'StageName-ASC', label: 'Stage (A-Z)', field: 'StageName', direction: 'ASC' },
    { value: 'LastModifiedDate-DESC', label: 'Last Modified (Recent First)', field: 'LastModifiedDate', direction: 'DESC' },
    { value: 'CreatedDate-DESC', label: 'Created Date (Recent First)', field: 'CreatedDate', direction: 'DESC' }
];

export default class OpportunityCards extends NavigationMixin(LightningElement) {
    @api recordId;
    @api displayMode = 'single';
    @api highlightFields = 'Amount,Probability';

    isLoading = true;
    error;
    wiredResult;
    fieldLabels = {};

    // Data
    openOpportunities = [];
    closedOpportunities = [];
    totalOpenCount = 0;
    totalClosedCount = 0;

    // Sort state
    currentSort = 'CloseDate-ASC';
    openSort = 'CloseDate-ASC';
    closedSort = 'CloseDate-ASC';

    // Mode checks
    get isSingleMode() { return this.displayMode === 'single'; }
    get isTabbedMode() { return this.displayMode === 'tabbed'; }
    get isMultiMode() { return this.displayMode === 'multi'; }

    get displayLimit() {
        return this.isSingleMode ? LIMIT_SINGLE : LIMIT_TABBED_MULTI;
    }

    get fieldList() {
        return this.highlightFields ? this.highlightFields.split(',').map(f => f.trim()) : [];
    }

    // Wire object info for field labels
    @wire(getObjectInfo, { objectApiName: OPPORTUNITY_OBJECT })
    wiredObjectInfo({ error, data }) {
        if (data) {
            this.fieldLabels = {};
            Object.keys(data.fields).forEach(fieldName => {
                this.fieldLabels[fieldName] = data.fields[fieldName].label;
            });
        }
    }

    // Wire data
    @wire(getOpportunities, {
        accountId: '$recordId',
        fields: '$fieldList',
        sortField: '$currentSortField',
        sortDirection: '$currentSortDirection',
        limitCount: '$wireLimitCount'
    })
    wiredOpportunities(result) {
        this.wiredResult = result;
        this.isLoading = false;

        if (result.error) {
            this.error = result.error;
            this.openOpportunities = [];
            this.closedOpportunities = [];
        } else if (result.data) {
            this.error = undefined;
            this.openOpportunities = result.data.openOpportunities || [];
            this.closedOpportunities = result.data.closedOpportunities || [];
            this.totalOpenCount = result.data.totalOpenCount || 0;
            this.totalClosedCount = result.data.totalClosedCount || 0;
        }
    }

    get wireLimitCount() {
        // For initial load, get enough for both sections
        return Math.max(LIMIT_SINGLE, LIMIT_TABBED_MULTI);
    }

    get currentSortField() {
        const option = BASE_SORT_OPTIONS.find(o => o.value === this.currentSort);
        return option ? option.field : 'CloseDate';
    }

    get currentSortDirection() {
        const option = BASE_SORT_OPTIONS.find(o => o.value === this.currentSort);
        return option ? option.direction : 'ASC';
    }

    get hasError() { return !!this.error; }
    get errorMessage() {
        if (!this.error) return '';
        return this.error.body?.message || this.error.message || 'An error occurred';
    }

    // Opportunity getters
    get allOpportunities() {
        return [...this.openOpportunities, ...this.closedOpportunities].slice(0, LIMIT_SINGLE);
    }

    get displayedOpenOpportunities() {
        return this.openOpportunities.slice(0, LIMIT_TABBED_MULTI);
    }

    get displayedClosedOpportunities() {
        return this.closedOpportunities.slice(0, LIMIT_TABBED_MULTI);
    }

    get hasOpportunities() {
        return this.totalOpenCount > 0 || this.totalClosedCount > 0;
    }

    get hasOpenOpportunities() {
        return this.totalOpenCount > 0;
    }

    get hasClosedOpportunities() {
        return this.totalClosedCount > 0;
    }

    // View All
    get showViewAllSingle() {
        return (this.totalOpenCount + this.totalClosedCount) > LIMIT_SINGLE;
    }

    get showViewAllOpen() {
        return this.totalOpenCount > LIMIT_TABBED_MULTI;
    }

    get showViewAllClosed() {
        return this.totalClosedCount > LIMIT_TABBED_MULTI;
    }

    get viewAllLabel() {
        const total = this.totalOpenCount + this.totalClosedCount;
        return `View All (${total})`;
    }

    get viewAllOpenLabel() {
        return `View All (${this.totalOpenCount})`;
    }

    get viewAllClosedLabel() {
        return `View All (${this.totalClosedCount})`;
    }

    // Tab labels
    get openTabLabel() {
        return `Open (${this.totalOpenCount})`;
    }

    get closedTabLabel() {
        return `Closed (${this.totalClosedCount})`;
    }

    // Sort options
    get sortOptions() {
        return this.buildSortOptions(this.currentSort);
    }

    get openSortOptions() {
        return this.buildSortOptions(this.openSort);
    }

    get closedSortOptions() {
        return this.buildSortOptions(this.closedSort);
    }

    buildSortOptions(currentValue) {
        const options = [...BASE_SORT_OPTIONS];

        // Add highlight field sort options
        this.fieldList.forEach(field => {
            if (!BASE_SORT_OPTIONS.some(o => o.field === field)) {
                options.push({
                    value: `${field}-ASC`,
                    label: `${this.fieldLabels[field] || field} (Asc)`,
                    field: field,
                    direction: 'ASC'
                });
                options.push({
                    value: `${field}-DESC`,
                    label: `${this.fieldLabels[field] || field} (Desc)`,
                    field: field,
                    direction: 'DESC'
                });
            }
        });

        return options.map(opt => ({
            ...opt,
            checked: opt.value === currentValue
        }));
    }

    // Event handlers
    handleSortChange(event) {
        this.currentSort = event.detail.value;
        this.handleRefresh();
    }

    handleOpenSortChange(event) {
        this.openSort = event.detail.value;
        // Re-sort locally for open opportunities
        this.sortOpportunities('open');
    }

    handleClosedSortChange(event) {
        this.closedSort = event.detail.value;
        // Re-sort locally for closed opportunities
        this.sortOpportunities('closed');
    }

    sortOpportunities(type) {
        const sortValue = type === 'open' ? this.openSort : this.closedSort;
        const option = BASE_SORT_OPTIONS.find(o => o.value === sortValue) ||
                       this.buildSortOptions(sortValue).find(o => o.value === sortValue);

        if (!option) return;

        const list = type === 'open' ? [...this.openOpportunities] : [...this.closedOpportunities];

        list.sort((a, b) => {
            let aVal = a[option.field] || a.fields?.[option.field];
            let bVal = b[option.field] || b.fields?.[option.field];

            if (aVal === null || aVal === undefined) aVal = '';
            if (bVal === null || bVal === undefined) bVal = '';

            let comparison = 0;
            if (typeof aVal === 'string') {
                comparison = aVal.localeCompare(bVal);
            } else {
                comparison = aVal < bVal ? -1 : (aVal > bVal ? 1 : 0);
            }

            return option.direction === 'DESC' ? -comparison : comparison;
        });

        if (type === 'open') {
            this.openOpportunities = list;
        } else {
            this.closedOpportunities = list;
        }
    }

    handleRefresh() {
        this.isLoading = true;
        refreshApex(this.wiredResult);
    }

    handleNew() {
        this[NavigationMixin.Navigate]({
            type: 'standard__objectPage',
            attributes: {
                objectApiName: 'Opportunity',
                actionName: 'new'
            },
            state: {
                defaultFieldValues: `AccountId=${this.recordId}`
            }
        });
    }

    // View All handlers
    handleViewAll() {
        this.openModal('all', [...this.openOpportunities, ...this.closedOpportunities]);
    }

    handleViewAllOpen() {
        // Tabbed mode navigates to related list
        this[NavigationMixin.Navigate]({
            type: 'standard__recordRelationshipPage',
            attributes: {
                recordId: this.recordId,
                objectApiName: 'Account',
                relationshipApiName: 'Opportunities',
                actionName: 'view'
            }
        });
    }

    handleViewAllClosed() {
        // Tabbed mode navigates to related list
        this[NavigationMixin.Navigate]({
            type: 'standard__recordRelationshipPage',
            attributes: {
                recordId: this.recordId,
                objectApiName: 'Account',
                relationshipApiName: 'Opportunities',
                actionName: 'view'
            }
        });
    }

    handleViewAllOpenModal() {
        this.openModal('open', this.openOpportunities);
    }

    handleViewAllClosedModal() {
        this.openModal('closed', this.closedOpportunities);
    }

    async openModal(type, opportunities) {
        await OpportunityCardsModal.open({
            size: 'large',
            label: type === 'all' ? 'All Opportunities' :
                   type === 'open' ? 'Open Opportunities' : 'Closed Opportunities',
            opportunities: opportunities,
            highlightFields: this.highlightFields,
            fieldLabels: this.fieldLabels,
            totalCount: type === 'all' ? (this.totalOpenCount + this.totalClosedCount) :
                        type === 'open' ? this.totalOpenCount : this.totalClosedCount
        });
    }
}
```

**Step 4: Create CSS**

```css
.cards-container {
    max-height: 70vh;
    overflow-y: auto;
}

.tab-actions {
    display: flex;
    justify-content: flex-end;
    padding-top: 0.5rem;
}
```

**Step 5: Commit**

```bash
git add force-app/main/default/lwc/opportunityCards
git commit -m "feat(lwc): add opportunityCards main container with all display modes"
```

---

## Phase 5: opportunityCardsModal LWC

### Task 9: Create opportunityCardsModal Component

**Files:**
- Create: `force-app/main/default/lwc/opportunityCardsModal/opportunityCardsModal.js`
- Create: `force-app/main/default/lwc/opportunityCardsModal/opportunityCardsModal.html`
- Create: `force-app/main/default/lwc/opportunityCardsModal/opportunityCardsModal.js-meta.xml`

**Step 1: Create meta.xml**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LightningComponentBundle xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>65.0</apiVersion>
    <isExposed>false</isExposed>
    <description>Modal for displaying all opportunities</description>
</LightningComponentBundle>
```

**Step 2: Create HTML template**

```html
<template>
    <lightning-modal-header label={modalLabel}></lightning-modal-header>
    <lightning-modal-body>
        <div class="modal-body-container">
            <template for:each={opportunities} for:item="opp">
                <c-opportunity-card
                    key={opp.id}
                    opportunity={opp}
                    highlight-fields={highlightFields}
                    field-labels={fieldLabels}>
                </c-opportunity-card>
            </template>
        </div>
    </lightning-modal-body>
    <lightning-modal-footer>
        <lightning-button label="Close" onclick={handleClose}></lightning-button>
    </lightning-modal-footer>
</template>
```

**Step 3: Create JavaScript**

```javascript
import { api } from 'lwc';
import LightningModal from 'lightning/modal';

export default class OpportunityCardsModal extends LightningModal {
    @api opportunities = [];
    @api highlightFields = '';
    @api fieldLabels = {};
    @api totalCount = 0;
    @api label = 'Opportunities';

    get modalLabel() {
        return `${this.label} (${this.totalCount})`;
    }

    handleClose() {
        this.close('closed');
    }
}
```

**Step 4: Commit**

```bash
git add force-app/main/default/lwc/opportunityCardsModal
git commit -m "feat(lwc): add opportunityCardsModal for View All functionality"
```

---

## Phase 6: Deploy & Test

### Task 10: Deploy All Components

**Step 1: Deploy to org**

```bash
sf project deploy start --source-dir force-app/main/default/classes/OpportunityCardsController.cls,force-app/main/default/classes/OpportunityCardsControllerTest.cls,force-app/main/default/lwc/productPillPopover,force-app/main/default/lwc/opportunityCard,force-app/main/default/lwc/opportunityCards,force-app/main/default/lwc/opportunityCardsModal --target-org <org-alias>
```

**Step 2: Run Apex tests and verify coverage**

```bash
sf apex run test --class-names OpportunityCardsControllerTest --result-format human --synchronous --code-coverage --target-org <org-alias>
```

Expected: All tests pass, coverage ≥90%

**Step 3: Add component to Account record page**

1. Open an Account record in Lightning
2. Edit Page
3. Drag "Opportunity Cards" component to page
4. Configure display mode and highlight fields
5. Save and Activate

**Step 4: Test all display modes**

- [ ] Single mode displays up to 15 opportunities
- [ ] Tabbed mode shows Open/Closed tabs with correct counts
- [ ] Multi mode shows two separate sections
- [ ] Product pills display with hover popovers
- [ ] Stage badges use correct colors (inverse/success/warning)
- [ ] Close date badges use correct colors (inverse/error)
- [ ] Sorting works in all modes
- [ ] New button creates opportunity with AccountId
- [ ] Edit button navigates to edit page
- [ ] View All modal opens correctly (Single/Multi)
- [ ] View All navigates to related list (Tabbed)
- [ ] Refresh button updates data

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat: complete Opportunity Cards LWC implementation

- Apex controller with wrapper classes and dynamic fields
- productPillPopover for line item display
- opportunityCard for individual card rendering
- opportunityCards container with single/tabbed/multi modes
- opportunityCardsModal for View All functionality
- Full test coverage (≥90%)"
```

---

## Summary

| Phase | Tasks | Description |
|-------|-------|-------------|
| 1 | 1-5 | Apex Controller with TDD |
| 2 | 6 | productPillPopover LWC |
| 3 | 7 | opportunityCard LWC |
| 4 | 8 | opportunityCards main LWC |
| 5 | 9 | opportunityCardsModal LWC |
| 6 | 10 | Deploy & Test |

**Total Tasks:** 10
**Estimated Commits:** 10
