# Add Products to Opportunity - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a custom "Add Products" LWC for Opportunities with reactive discount calculation.

**Architecture:** Two-screen wizard (search → edit) orchestrated by parent component. Apex controller handles PricebookEntry queries and OpportunityLineItem creation. Discount validation ensures Sales Price never goes negative.

**Tech Stack:** LWC, Apex, SLDS, lightning-datatable

---

## Task 1: Create Apex Controller

**Files:**
- Create: `force-app/main/default/classes/AddProductsController.cls`
- Create: `force-app/main/default/classes/AddProductsController.cls-meta.xml`

**Step 1: Create the controller with wrapper classes**

```apex
/**
 * Controller for custom Add Products LWC.
 * Handles product search and OpportunityLineItem creation with discount support.
 */
public with sharing class AddProductsController {

    /**
     * Get the Opportunity's assigned Pricebook info.
     * @param opportunityId The Opportunity record Id
     * @return PricebookInfo wrapper with Id and Name
     */
    @AuraEnabled(cacheable=true)
    public static PricebookInfo getOpportunityPricebook(Id opportunityId) {
        if (opportunityId == null) {
            throw new AuraHandledException('Opportunity Id is required');
        }

        List<Opportunity> opps = [
            SELECT Id, Pricebook2Id, Pricebook2.Name
            FROM Opportunity
            WHERE Id = :opportunityId
            WITH USER_MODE
            LIMIT 1
        ];

        if (opps.isEmpty()) {
            throw new AuraHandledException('Opportunity not found');
        }

        Opportunity opp = opps[0];
        if (opp.Pricebook2Id == null) {
            throw new AuraHandledException('No Price Book assigned to this Opportunity. Please assign a Price Book first.');
        }

        return new PricebookInfo(opp.Pricebook2Id, opp.Pricebook2.Name);
    }

    /**
     * Search products from the Opportunity's Pricebook.
     * @param opportunityId The Opportunity record Id (to get Pricebook)
     * @param searchTerm Optional search string for Product Name/Code
     * @param familyFilter Optional Product Family filter
     * @return List of ProductWrapper records
     */
    @AuraEnabled(cacheable=true)
    public static List<ProductWrapper> getProducts(
        Id opportunityId,
        String searchTerm,
        String familyFilter
    ) {
        if (opportunityId == null) {
            throw new AuraHandledException('Opportunity Id is required');
        }

        // Get Pricebook from Opportunity
        List<Opportunity> opps = [
            SELECT Pricebook2Id
            FROM Opportunity
            WHERE Id = :opportunityId
            WITH USER_MODE
            LIMIT 1
        ];

        if (opps.isEmpty() || opps[0].Pricebook2Id == null) {
            throw new AuraHandledException('No Price Book assigned to this Opportunity');
        }

        Id pricebookId = opps[0].Pricebook2Id;

        // Build dynamic query
        String query = 'SELECT Id, Product2Id, Product2.Name, Product2.ProductCode, ' +
                       'Product2.Description, Product2.Family, UnitPrice, IsActive ' +
                       'FROM PricebookEntry ' +
                       'WHERE Pricebook2Id = :pricebookId AND IsActive = true';

        // Add search filter
        String searchPattern = '';
        if (String.isNotBlank(searchTerm)) {
            searchPattern = '%' + String.escapeSingleQuotes(searchTerm.trim()) + '%';
            query += ' AND (Product2.Name LIKE :searchPattern OR Product2.ProductCode LIKE :searchPattern)';
        }

        // Add family filter
        String familySafe = '';
        if (String.isNotBlank(familyFilter)) {
            familySafe = String.escapeSingleQuotes(familyFilter.trim());
            query += ' AND Product2.Family = :familySafe';
        }

        query += ' ORDER BY Product2.Name ASC LIMIT 200';

        List<PricebookEntry> entries = Database.query(query);

        List<ProductWrapper> results = new List<ProductWrapper>();
        for (PricebookEntry pbe : entries) {
            results.add(new ProductWrapper(pbe));
        }

        return results;
    }

    /**
     * Get available Product Family picklist values.
     * @return List of family options
     */
    @AuraEnabled(cacheable=true)
    public static List<PicklistOption> getProductFamilies() {
        List<PicklistOption> options = new List<PicklistOption>();
        options.add(new PicklistOption('', 'All Families'));

        Schema.DescribeFieldResult fieldResult = Product2.Family.getDescribe();
        for (Schema.PicklistEntry entry : fieldResult.getPicklistValues()) {
            if (entry.isActive()) {
                options.add(new PicklistOption(entry.getValue(), entry.getLabel()));
            }
        }

        return options;
    }

    /**
     * Save OpportunityLineItems with discount values.
     * @param opportunityId The Opportunity record Id
     * @param lineItemsJson JSON string of LineItemInput list
     */
    @AuraEnabled
    public static void saveLineItems(Id opportunityId, String lineItemsJson) {
        if (opportunityId == null) {
            throw new AuraHandledException('Opportunity Id is required');
        }

        if (String.isBlank(lineItemsJson)) {
            throw new AuraHandledException('No line items provided');
        }

        List<LineItemInput> inputs;
        try {
            inputs = (List<LineItemInput>) JSON.deserialize(lineItemsJson, List<LineItemInput>.class);
        } catch (Exception e) {
            throw new AuraHandledException('Invalid line item data: ' + e.getMessage());
        }

        if (inputs.isEmpty()) {
            throw new AuraHandledException('No line items provided');
        }

        List<OpportunityLineItem> oliList = new List<OpportunityLineItem>();

        for (LineItemInput input : inputs) {
            // Validate discount
            Decimal discount = input.discountValue != null ? input.discountValue : 0;
            if (discount < 0) {
                throw new AuraHandledException('Discount cannot be negative');
            }
            if (discount > input.listPrice) {
                throw new AuraHandledException('Discount cannot exceed List Price');
            }

            // Calculate UnitPrice
            Decimal unitPrice = input.listPrice - discount;

            OpportunityLineItem oli = new OpportunityLineItem();
            oli.OpportunityId = opportunityId;
            oli.PricebookEntryId = input.pricebookEntryId;
            oli.Quantity = input.quantity != null ? input.quantity : 1;
            oli.UnitPrice = unitPrice;
            oli.Discount_Value__c = discount;
            oli.ServiceDate = input.serviceDate;
            oli.Description = input.description;

            oliList.add(oli);
        }

        try {
            insert oliList;
        } catch (DmlException e) {
            throw new AuraHandledException('Failed to save line items: ' + e.getMessage());
        }
    }

    // ===== WRAPPER CLASSES =====

    public class PricebookInfo {
        @AuraEnabled public Id pricebookId;
        @AuraEnabled public String pricebookName;

        public PricebookInfo(Id pricebookId, String pricebookName) {
            this.pricebookId = pricebookId;
            this.pricebookName = pricebookName;
        }
    }

    public class ProductWrapper {
        @AuraEnabled public Id pricebookEntryId;
        @AuraEnabled public Id product2Id;
        @AuraEnabled public String productName;
        @AuraEnabled public String productCode;
        @AuraEnabled public Decimal listPrice;
        @AuraEnabled public String description;
        @AuraEnabled public String family;

        public ProductWrapper(PricebookEntry pbe) {
            this.pricebookEntryId = pbe.Id;
            this.product2Id = pbe.Product2Id;
            this.productName = pbe.Product2.Name;
            this.productCode = pbe.Product2.ProductCode;
            this.listPrice = pbe.UnitPrice;
            this.description = pbe.Product2.Description;
            this.family = pbe.Product2.Family;
        }
    }

    public class LineItemInput {
        @AuraEnabled public Id pricebookEntryId;
        @AuraEnabled public Decimal listPrice;
        @AuraEnabled public Integer quantity;
        @AuraEnabled public Decimal discountValue;
        @AuraEnabled public Date serviceDate;
        @AuraEnabled public String description;
    }

    public class PicklistOption {
        @AuraEnabled public String value;
        @AuraEnabled public String label;

        public PicklistOption(String value, String label) {
            this.value = value;
            this.label = label;
        }
    }
}
```

**Step 2: Create the meta.xml file**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<ApexClass xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>62.0</apiVersion>
    <status>Active</status>
</ApexClass>
```

---

## Task 2: Create Apex Test Class

**Files:**
- Create: `force-app/main/default/classes/AddProductsControllerTest.cls`
- Create: `force-app/main/default/classes/AddProductsControllerTest.cls-meta.xml`

**Step 1: Create comprehensive test class (90%+ coverage)**

```apex
/**
 * Test class for AddProductsController.
 * Covers all methods with positive and negative scenarios.
 */
@IsTest
private class AddProductsControllerTest {

    @TestSetup
    static void setupTestData() {
        // Create Pricebook - use Standard Pricebook
        Id stdPricebookId = Test.getStandardPricebookId();

        // Create Products
        List<Product2> products = new List<Product2>();
        for (Integer i = 1; i <= 5; i++) {
            products.add(new Product2(
                Name = 'Test Product ' + i,
                ProductCode = 'TP00' + i,
                Description = 'Description for product ' + i,
                Family = (Math.mod(i, 2) == 0) ? 'Software' : 'Services',
                IsActive = true
            ));
        }
        insert products;

        // Create Standard PricebookEntries
        List<PricebookEntry> stdEntries = new List<PricebookEntry>();
        for (Product2 prod : products) {
            stdEntries.add(new PricebookEntry(
                Pricebook2Id = stdPricebookId,
                Product2Id = prod.Id,
                UnitPrice = 100.00 * products.indexOf(prod) + 100,
                IsActive = true
            ));
        }
        insert stdEntries;

        // Create Account and Opportunity
        Account acc = new Account(Name = 'Test Account');
        insert acc;

        Opportunity opp = new Opportunity(
            Name = 'Test Opportunity',
            AccountId = acc.Id,
            StageName = 'Prospecting',
            CloseDate = Date.today().addDays(30),
            Pricebook2Id = stdPricebookId
        );
        insert opp;

        // Create Opportunity without Pricebook for negative test
        Opportunity oppNoPb = new Opportunity(
            Name = 'Test Opportunity No PB',
            AccountId = acc.Id,
            StageName = 'Prospecting',
            CloseDate = Date.today().addDays(30)
        );
        insert oppNoPb;
    }

    @IsTest
    static void testGetOpportunityPricebook_Success() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];

        Test.startTest();
        AddProductsController.PricebookInfo result = AddProductsController.getOpportunityPricebook(opp.Id);
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.assertNotEquals(null, result.pricebookId, 'Pricebook Id should not be null');
        System.assertEquals('Standard Price Book', result.pricebookName, 'Should return Standard Price Book');
    }

    @IsTest
    static void testGetOpportunityPricebook_NullId() {
        Test.startTest();
        try {
            AddProductsController.getOpportunityPricebook(null);
            System.assert(false, 'Should have thrown exception');
        } catch (AuraHandledException e) {
            System.assert(e.getMessage().contains('required'), 'Should indicate Id is required');
        }
        Test.stopTest();
    }

    @IsTest
    static void testGetOpportunityPricebook_NoPricebook() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity No PB' LIMIT 1];

        Test.startTest();
        try {
            AddProductsController.getOpportunityPricebook(opp.Id);
            System.assert(false, 'Should have thrown exception');
        } catch (AuraHandledException e) {
            System.assert(e.getMessage().contains('No Price Book'), 'Should indicate no pricebook');
        }
        Test.stopTest();
    }

    @IsTest
    static void testGetProducts_Success() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];

        Test.startTest();
        List<AddProductsController.ProductWrapper> results = AddProductsController.getProducts(opp.Id, null, null);
        Test.stopTest();

        System.assertEquals(5, results.size(), 'Should return 5 products');
        System.assertNotEquals(null, results[0].pricebookEntryId, 'PricebookEntryId should be set');
        System.assertNotEquals(null, results[0].productName, 'Product name should be set');
    }

    @IsTest
    static void testGetProducts_WithSearchTerm() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];

        Test.startTest();
        List<AddProductsController.ProductWrapper> results = AddProductsController.getProducts(opp.Id, 'Product 1', null);
        Test.stopTest();

        System.assertEquals(1, results.size(), 'Should return 1 matching product');
        System.assert(results[0].productName.contains('1'), 'Should match search term');
    }

    @IsTest
    static void testGetProducts_WithFamilyFilter() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];

        Test.startTest();
        List<AddProductsController.ProductWrapper> results = AddProductsController.getProducts(opp.Id, null, 'Software');
        Test.stopTest();

        System.assertEquals(2, results.size(), 'Should return 2 Software products');
        for (AddProductsController.ProductWrapper pw : results) {
            System.assertEquals('Software', pw.family, 'All should be Software family');
        }
    }

    @IsTest
    static void testGetProducts_NoPricebook() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity No PB' LIMIT 1];

        Test.startTest();
        try {
            AddProductsController.getProducts(opp.Id, null, null);
            System.assert(false, 'Should have thrown exception');
        } catch (AuraHandledException e) {
            System.assert(e.getMessage().contains('No Price Book'), 'Should indicate no pricebook');
        }
        Test.stopTest();
    }

    @IsTest
    static void testGetProductFamilies() {
        Test.startTest();
        List<AddProductsController.PicklistOption> options = AddProductsController.getProductFamilies();
        Test.stopTest();

        System.assert(options.size() > 0, 'Should return picklist options');
        System.assertEquals('', options[0].value, 'First option should be All Families');
    }

    @IsTest
    static void testSaveLineItems_Success() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];
        PricebookEntry pbe = [SELECT Id, UnitPrice FROM PricebookEntry WHERE Product2.Name = 'Test Product 1' LIMIT 1];

        AddProductsController.LineItemInput input = new AddProductsController.LineItemInput();
        input.pricebookEntryId = pbe.Id;
        input.listPrice = pbe.UnitPrice;
        input.quantity = 2;
        input.discountValue = 10.00;
        input.serviceDate = Date.today();
        input.description = 'Test description';

        String jsonInput = JSON.serialize(new List<AddProductsController.LineItemInput>{ input });

        Test.startTest();
        AddProductsController.saveLineItems(opp.Id, jsonInput);
        Test.stopTest();

        List<OpportunityLineItem> olis = [SELECT Id, Quantity, UnitPrice, Discount_Value__c, Description FROM OpportunityLineItem WHERE OpportunityId = :opp.Id];
        System.assertEquals(1, olis.size(), 'Should create 1 line item');
        System.assertEquals(2, olis[0].Quantity, 'Quantity should be 2');
        System.assertEquals(pbe.UnitPrice - 10.00, olis[0].UnitPrice, 'UnitPrice should be ListPrice - Discount');
        System.assertEquals(10.00, olis[0].Discount_Value__c, 'Discount should be saved');
    }

    @IsTest
    static void testSaveLineItems_BulkInsert() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];
        List<PricebookEntry> pbes = [SELECT Id, UnitPrice FROM PricebookEntry LIMIT 5];

        List<AddProductsController.LineItemInput> inputs = new List<AddProductsController.LineItemInput>();
        for (PricebookEntry pbe : pbes) {
            AddProductsController.LineItemInput input = new AddProductsController.LineItemInput();
            input.pricebookEntryId = pbe.Id;
            input.listPrice = pbe.UnitPrice;
            input.quantity = 1;
            input.discountValue = 0;
            inputs.add(input);
        }

        String jsonInput = JSON.serialize(inputs);

        Test.startTest();
        AddProductsController.saveLineItems(opp.Id, jsonInput);
        Test.stopTest();

        Integer count = [SELECT COUNT() FROM OpportunityLineItem WHERE OpportunityId = :opp.Id];
        System.assertEquals(5, count, 'Should create 5 line items');
    }

    @IsTest
    static void testSaveLineItems_NegativeDiscount() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];
        PricebookEntry pbe = [SELECT Id, UnitPrice FROM PricebookEntry LIMIT 1];

        AddProductsController.LineItemInput input = new AddProductsController.LineItemInput();
        input.pricebookEntryId = pbe.Id;
        input.listPrice = pbe.UnitPrice;
        input.quantity = 1;
        input.discountValue = -10.00;

        String jsonInput = JSON.serialize(new List<AddProductsController.LineItemInput>{ input });

        Test.startTest();
        try {
            AddProductsController.saveLineItems(opp.Id, jsonInput);
            System.assert(false, 'Should have thrown exception');
        } catch (AuraHandledException e) {
            System.assert(e.getMessage().contains('negative'), 'Should indicate negative discount error');
        }
        Test.stopTest();
    }

    @IsTest
    static void testSaveLineItems_DiscountExceedsPrice() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];
        PricebookEntry pbe = [SELECT Id, UnitPrice FROM PricebookEntry LIMIT 1];

        AddProductsController.LineItemInput input = new AddProductsController.LineItemInput();
        input.pricebookEntryId = pbe.Id;
        input.listPrice = pbe.UnitPrice;
        input.quantity = 1;
        input.discountValue = pbe.UnitPrice + 100;

        String jsonInput = JSON.serialize(new List<AddProductsController.LineItemInput>{ input });

        Test.startTest();
        try {
            AddProductsController.saveLineItems(opp.Id, jsonInput);
            System.assert(false, 'Should have thrown exception');
        } catch (AuraHandledException e) {
            System.assert(e.getMessage().contains('exceed'), 'Should indicate discount exceeds price');
        }
        Test.stopTest();
    }

    @IsTest
    static void testSaveLineItems_NullOpportunityId() {
        Test.startTest();
        try {
            AddProductsController.saveLineItems(null, '[]');
            System.assert(false, 'Should have thrown exception');
        } catch (AuraHandledException e) {
            System.assert(e.getMessage().contains('required'), 'Should indicate Id is required');
        }
        Test.stopTest();
    }

    @IsTest
    static void testSaveLineItems_EmptyJson() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];

        Test.startTest();
        try {
            AddProductsController.saveLineItems(opp.Id, '');
            System.assert(false, 'Should have thrown exception');
        } catch (AuraHandledException e) {
            System.assert(e.getMessage().contains('No line items'), 'Should indicate no items');
        }
        Test.stopTest();
    }

    @IsTest
    static void testSaveLineItems_InvalidJson() {
        Opportunity opp = [SELECT Id FROM Opportunity WHERE Name = 'Test Opportunity' LIMIT 1];

        Test.startTest();
        try {
            AddProductsController.saveLineItems(opp.Id, 'invalid json');
            System.assert(false, 'Should have thrown exception');
        } catch (AuraHandledException e) {
            System.assert(e.getMessage().contains('Invalid'), 'Should indicate invalid data');
        }
        Test.stopTest();
    }
}
```

**Step 2: Create the meta.xml file**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<ApexClass xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>62.0</apiVersion>
    <status>Active</status>
</ApexClass>
```

**Step 3: Deploy and verify coverage**

Run: `sf project deploy start --source-dir force-app/main/default/classes/AddProductsController.cls,force-app/main/default/classes/AddProductsControllerTest.cls --target-org <alias>`

Then run tests:
Run: `sf apex run test --class-names AddProductsControllerTest --code-coverage --result-format human --target-org <alias>`

Expected: 90%+ code coverage on AddProductsController

---

## Task 3: Create Parent LWC (addProductsToOpportunity)

**Files:**
- Create: `force-app/main/default/lwc/addProductsToOpportunity/addProductsToOpportunity.js`
- Create: `force-app/main/default/lwc/addProductsToOpportunity/addProductsToOpportunity.html`
- Create: `force-app/main/default/lwc/addProductsToOpportunity/addProductsToOpportunity.css`
- Create: `force-app/main/default/lwc/addProductsToOpportunity/addProductsToOpportunity.js-meta.xml`

**Step 1: Create the JavaScript controller**

```javascript
import { LightningElement, api, track } from "lwc";
import { ShowToastEvent } from "lightning/platformShowToastEvent";
import { CloseActionScreenEvent } from "lightning/actions";
import { FlowNavigationFinishEvent } from "lightning/flowSupport";
import getOpportunityPricebook from "@salesforce/apex/AddProductsController.getOpportunityPricebook";

const SCREEN_SEARCH = "search";
const SCREEN_EDIT = "edit";

export default class AddProductsToOpportunity extends LightningElement {
  // Record page context
  @api recordId;

  // Flow context
  @api opportunityId;

  // Flow output
  @api lineItemsCreated = 0;

  @track currentScreen = SCREEN_SEARCH;
  @track pricebookInfo;
  @track selectedProducts = [];
  @track isLoading = true;
  @track errorMessage;

  get effectiveOpportunityId() {
    return this.opportunityId || this.recordId;
  }

  get isSearchScreen() {
    return this.currentScreen === SCREEN_SEARCH;
  }

  get isEditScreen() {
    return this.currentScreen === SCREEN_EDIT;
  }

  get pricebookName() {
    return this.pricebookInfo?.pricebookName || "Loading...";
  }

  get hasError() {
    return !!this.errorMessage;
  }

  connectedCallback() {
    this.loadPricebook();
  }

  async loadPricebook() {
    this.isLoading = true;
    this.errorMessage = null;

    try {
      this.pricebookInfo = await getOpportunityPricebook({
        opportunityId: this.effectiveOpportunityId,
      });
    } catch (error) {
      this.errorMessage =
        error.body?.message || "Failed to load Price Book information";
      this.showToast("Error", this.errorMessage, "error");
    } finally {
      this.isLoading = false;
    }
  }

  handleProductsSelected(event) {
    this.selectedProducts = event.detail.products;
    this.currentScreen = SCREEN_EDIT;
  }

  handleBack() {
    this.currentScreen = SCREEN_SEARCH;
  }

  handleCancel() {
    // Close action modal if on record page
    this.dispatchEvent(new CloseActionScreenEvent());

    // Navigate finish if in flow
    this.dispatchEvent(new FlowNavigationFinishEvent());
  }

  handleSaveComplete(event) {
    this.lineItemsCreated = event.detail.count;

    this.showToast(
      "Success",
      `${this.lineItemsCreated} product(s) added successfully`,
      "success"
    );

    // Close action modal
    this.dispatchEvent(new CloseActionScreenEvent());

    // Navigate finish if in flow
    this.dispatchEvent(new FlowNavigationFinishEvent());
  }

  handleError(event) {
    this.showToast("Error", event.detail.message, "error");
  }

  showToast(title, message, variant) {
    this.dispatchEvent(
      new ShowToastEvent({
        title,
        message,
        variant,
      })
    );
  }
}
```

**Step 2: Create the HTML template**

```html
<template>
  <lightning-card>
    <!-- Header -->
    <div slot="title" class="slds-text-heading_medium">
      Add Products
      <template lwc:if={pricebookInfo}>
        <span class="slds-text-body_small slds-text-color_weak">
          Price Book: {pricebookName}
        </span>
      </template>
    </div>

    <!-- Error State -->
    <template lwc:if={hasError}>
      <div class="slds-p-around_medium">
        <div
          class="slds-notify slds-notify_alert slds-alert_error"
          role="alert"
        >
          <span class="slds-assistive-text">error</span>
          <h2>{errorMessage}</h2>
        </div>
      </div>
    </template>

    <!-- Loading State -->
    <template lwc:elseif={isLoading}>
      <div class="slds-p-around_large slds-align_absolute-center">
        <lightning-spinner
          alternative-text="Loading"
          size="medium"
        ></lightning-spinner>
      </div>
    </template>

    <!-- Search Screen -->
    <template lwc:elseif={isSearchScreen}>
      <c-add-products-search
        opportunity-id={effectiveOpportunityId}
        pricebook-id={pricebookInfo.pricebookId}
        onproductsselected={handleProductsSelected}
        oncancel={handleCancel}
        onerror={handleError}
      >
      </c-add-products-search>
    </template>

    <!-- Edit Screen -->
    <template lwc:elseif={isEditScreen}>
      <c-add-products-edit
        opportunity-id={effectiveOpportunityId}
        selected-products={selectedProducts}
        onback={handleBack}
        oncancel={handleCancel}
        onsavecomplete={handleSaveComplete}
        onerror={handleError}
      >
      </c-add-products-edit>
    </template>
  </lightning-card>
</template>
```

**Step 3: Create the CSS file**

```css
:host {
  display: block;
}

.slds-text-color_weak {
  color: #706e6b;
  font-weight: normal;
  margin-left: 0.5rem;
}
```

**Step 4: Create the meta.xml file**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LightningComponentBundle xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>62.0</apiVersion>
    <isExposed>true</isExposed>
    <masterLabel>Add Products to Opportunity</masterLabel>
    <description>Custom Add Products wizard with discount support</description>
    <targets>
        <target>lightning__RecordAction</target>
        <target>lightning__RecordPage</target>
        <target>lightning__FlowScreen</target>
    </targets>
    <targetConfigs>
        <targetConfig targets="lightning__RecordAction">
            <actionType>ScreenAction</actionType>
        </targetConfig>
        <targetConfig targets="lightning__RecordPage">
            <objects>
                <object>Opportunity</object>
            </objects>
        </targetConfig>
        <targetConfig targets="lightning__FlowScreen">
            <property name="opportunityId" type="String" label="Opportunity Id" description="The Opportunity to add products to" role="inputOnly"/>
            <property name="lineItemsCreated" type="Integer" label="Line Items Created" description="Number of line items created" role="outputOnly"/>
        </targetConfig>
    </targetConfigs>
</LightningComponentBundle>
```

---

## Task 4: Create Search LWC (addProductsSearch)

**Files:**
- Create: `force-app/main/default/lwc/addProductsSearch/addProductsSearch.js`
- Create: `force-app/main/default/lwc/addProductsSearch/addProductsSearch.html`
- Create: `force-app/main/default/lwc/addProductsSearch/addProductsSearch.css`
- Create: `force-app/main/default/lwc/addProductsSearch/addProductsSearch.js-meta.xml`

**Step 1: Create the JavaScript controller**

```javascript
import { LightningElement, api, track, wire } from "lwc";
import getProducts from "@salesforce/apex/AddProductsController.getProducts";
import getProductFamilies from "@salesforce/apex/AddProductsController.getProductFamilies";

const COLUMNS = [
  {
    label: "Product Name",
    fieldName: "productName",
    type: "text",
    sortable: true,
  },
  {
    label: "Product Code",
    fieldName: "productCode",
    type: "text",
    sortable: true,
  },
  {
    label: "List Price",
    fieldName: "listPrice",
    type: "currency",
    sortable: true,
    cellAttributes: { alignment: "left" },
  },
  { label: "Description", fieldName: "description", type: "text" },
  { label: "Product Family", fieldName: "family", type: "text", sortable: true },
];

const SEARCH_DELAY = 300;

export default class AddProductsSearch extends LightningElement {
  @api opportunityId;
  @api pricebookId;

  @track products = [];
  @track filteredProducts = [];
  @track selectedRows = [];
  @track familyOptions = [];

  columns = COLUMNS;
  searchTerm = "";
  familyFilter = "";
  showSelectedOnly = false;
  isLoading = false;
  searchTimeout;

  sortedBy;
  sortedDirection = "asc";

  @wire(getProductFamilies)
  wiredFamilies({ error, data }) {
    if (data) {
      this.familyOptions = data;
    } else if (error) {
      this.dispatchError("Failed to load product families");
    }
  }

  get selectedCount() {
    return this.selectedRows.length;
  }

  get showSelectedLabel() {
    return `Show Selected (${this.selectedCount})`;
  }

  get hasSelection() {
    return this.selectedCount > 0;
  }

  get isNextDisabled() {
    return !this.hasSelection;
  }

  get displayedProducts() {
    if (this.showSelectedOnly) {
      const selectedIds = new Set(this.selectedRows);
      return this.filteredProducts.filter((p) =>
        selectedIds.has(p.pricebookEntryId)
      );
    }
    return this.filteredProducts;
  }

  connectedCallback() {
    this.loadProducts();
  }

  async loadProducts() {
    this.isLoading = true;

    try {
      const results = await getProducts({
        opportunityId: this.opportunityId,
        searchTerm: this.searchTerm,
        familyFilter: this.familyFilter,
      });

      this.products = results.map((p) => ({
        ...p,
        id: p.pricebookEntryId, // For datatable row selection
      }));
      this.filteredProducts = [...this.products];
      this.applySorting();
    } catch (error) {
      this.dispatchError(error.body?.message || "Failed to load products");
      this.products = [];
      this.filteredProducts = [];
    } finally {
      this.isLoading = false;
    }
  }

  handleSearchChange(event) {
    this.searchTerm = event.target.value;

    clearTimeout(this.searchTimeout);
    this.searchTimeout = setTimeout(() => {
      this.loadProducts();
    }, SEARCH_DELAY);
  }

  handleFamilyChange(event) {
    this.familyFilter = event.detail.value;
    this.loadProducts();
  }

  handleShowSelectedChange(event) {
    this.showSelectedOnly = event.target.checked;
  }

  handleRowSelection(event) {
    this.selectedRows = event.detail.selectedRows.map(
      (row) => row.pricebookEntryId
    );
  }

  handleSort(event) {
    this.sortedBy = event.detail.fieldName;
    this.sortedDirection = event.detail.sortDirection;
    this.applySorting();
  }

  applySorting() {
    if (!this.sortedBy) return;

    const data = [...this.filteredProducts];
    const reverse = this.sortedDirection === "asc" ? 1 : -1;

    data.sort((a, b) => {
      let valueA = a[this.sortedBy] || "";
      let valueB = b[this.sortedBy] || "";

      if (typeof valueA === "string") {
        valueA = valueA.toLowerCase();
        valueB = valueB.toLowerCase();
      }

      if (valueA < valueB) return -1 * reverse;
      if (valueA > valueB) return 1 * reverse;
      return 0;
    });

    this.filteredProducts = data;
  }

  handleCancel() {
    this.dispatchEvent(new CustomEvent("cancel"));
  }

  handleNext() {
    const selectedProducts = this.products.filter((p) =>
      this.selectedRows.includes(p.pricebookEntryId)
    );

    this.dispatchEvent(
      new CustomEvent("productsselected", {
        detail: { products: selectedProducts },
      })
    );
  }

  dispatchError(message) {
    this.dispatchEvent(
      new CustomEvent("error", {
        detail: { message },
      })
    );
  }
}
```

**Step 2: Create the HTML template**

```html
<template>
  <div class="slds-p-around_medium">
    <!-- Search and Filter Row -->
    <div class="slds-grid slds-gutters slds-m-bottom_small">
      <div class="slds-col slds-size_1-of-2">
        <lightning-input
          type="search"
          label="Search Products"
          variant="label-hidden"
          placeholder="Search Products..."
          value={searchTerm}
          onchange={handleSearchChange}
        >
        </lightning-input>
      </div>
      <div class="slds-col slds-size_1-of-4">
        <lightning-combobox
          label="Product Family"
          variant="label-hidden"
          placeholder="All Families"
          options={familyOptions}
          value={familyFilter}
          onchange={handleFamilyChange}
        >
        </lightning-combobox>
      </div>
      <div class="slds-col slds-size_1-of-4 slds-align-middle">
        <lightning-input
          type="checkbox"
          label={showSelectedLabel}
          checked={showSelectedOnly}
          onchange={handleShowSelectedChange}
          disabled={!hasSelection}
        >
        </lightning-input>
      </div>
    </div>

    <!-- Products Datatable -->
    <div class="datatable-container">
      <template lwc:if={isLoading}>
        <div class="slds-is-relative loading-overlay">
          <lightning-spinner
            alternative-text="Loading"
            size="small"
          ></lightning-spinner>
        </div>
      </template>

      <lightning-datatable
        key-field="pricebookEntryId"
        data={displayedProducts}
        columns={columns}
        selected-rows={selectedRows}
        onrowselection={handleRowSelection}
        onsort={handleSort}
        sorted-by={sortedBy}
        sorted-direction={sortedDirection}
        show-row-number-column
        max-row-selection="200"
      >
      </lightning-datatable>
    </div>

    <!-- Footer Buttons -->
    <div class="slds-m-top_medium slds-clearfix">
      <div class="slds-float_right">
        <lightning-button
          label="Cancel"
          onclick={handleCancel}
          class="slds-m-right_x-small"
        >
        </lightning-button>
        <lightning-button
          label="Next"
          variant="brand"
          onclick={handleNext}
          disabled={isNextDisabled}
        >
        </lightning-button>
      </div>
    </div>
  </div>
</template>
```

**Step 3: Create the CSS file**

```css
:host {
  display: block;
}

.datatable-container {
  min-height: 300px;
  max-height: 400px;
  overflow-y: auto;
  position: relative;
}

.loading-overlay {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  z-index: 1;
}
```

**Step 4: Create the meta.xml file**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LightningComponentBundle xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>62.0</apiVersion>
    <isExposed>false</isExposed>
    <masterLabel>Add Products Search</masterLabel>
    <description>Product search and selection screen for Add Products wizard</description>
</LightningComponentBundle>
```

---

## Task 5: Create Edit LWC (addProductsEdit)

**Files:**
- Create: `force-app/main/default/lwc/addProductsEdit/addProductsEdit.js`
- Create: `force-app/main/default/lwc/addProductsEdit/addProductsEdit.html`
- Create: `force-app/main/default/lwc/addProductsEdit/addProductsEdit.css`
- Create: `force-app/main/default/lwc/addProductsEdit/addProductsEdit.js-meta.xml`

**Step 1: Create the JavaScript controller**

```javascript
import { LightningElement, api, track } from "lwc";
import saveLineItems from "@salesforce/apex/AddProductsController.saveLineItems";

export default class AddProductsEdit extends LightningElement {
  @api opportunityId;

  @api
  get selectedProducts() {
    return this._selectedProducts;
  }
  set selectedProducts(value) {
    this._selectedProducts = value;
    this.initializeLineItems();
  }

  @track lineItems = [];
  @track isSaving = false;

  _selectedProducts = [];

  initializeLineItems() {
    if (!this._selectedProducts || this._selectedProducts.length === 0) {
      this.lineItems = [];
      return;
    }

    this.lineItems = this._selectedProducts.map((product, index) => ({
      key: `line-${index}-${Date.now()}`,
      rowNumber: index + 1,
      pricebookEntryId: product.pricebookEntryId,
      productName: product.productName,
      listPrice: product.listPrice,
      quantity: 1,
      discountValue: 0,
      salesPrice: product.listPrice,
      serviceDate: null,
      description: "",
      quantityError: "",
      discountError: "",
    }));
  }

  get hasLineItems() {
    return this.lineItems.length > 0;
  }

  get isSaveDisabled() {
    return (
      this.isSaving || !this.hasLineItems || !this.validateAllRows()
    );
  }

  validateAllRows() {
    return this.lineItems.every(
      (item) => !item.quantityError && !item.discountError && item.quantity >= 1
    );
  }

  handleQuantityChange(event) {
    const index = parseInt(event.target.dataset.index, 10);
    const value = parseInt(event.target.value, 10);

    const item = { ...this.lineItems[index] };

    if (isNaN(value) || value < 1) {
      item.quantityError = "Quantity must be at least 1";
    } else {
      item.quantity = value;
      item.quantityError = "";
    }

    this.updateLineItem(index, item);
  }

  handleDiscountChange(event) {
    const index = parseInt(event.target.dataset.index, 10);
    const value = parseFloat(event.target.value);

    const item = { ...this.lineItems[index] };
    const discount = isNaN(value) ? 0 : value;

    // Validate discount
    if (discount < 0) {
      item.discountError = "Discount cannot be negative";
    } else if (discount > item.listPrice) {
      item.discountError = `Discount cannot exceed $${item.listPrice.toFixed(2)}`;
    } else {
      item.discountValue = discount;
      item.salesPrice = item.listPrice - discount;
      item.discountError = "";
    }

    this.updateLineItem(index, item);
  }

  handleDateChange(event) {
    const index = parseInt(event.target.dataset.index, 10);
    const value = event.target.value;

    const item = { ...this.lineItems[index] };
    item.serviceDate = value || null;

    this.updateLineItem(index, item);
  }

  handleDescriptionChange(event) {
    const index = parseInt(event.target.dataset.index, 10);
    const value = event.target.value;

    const item = { ...this.lineItems[index] };
    item.description = value;

    this.updateLineItem(index, item);
  }

  handleDeleteRow(event) {
    const index = parseInt(event.target.dataset.index, 10);

    this.lineItems = this.lineItems
      .filter((_, i) => i !== index)
      .map((item, i) => ({
        ...item,
        rowNumber: i + 1,
      }));
  }

  updateLineItem(index, item) {
    this.lineItems = this.lineItems.map((existingItem, i) =>
      i === index ? item : existingItem
    );
  }

  formatCurrency(value) {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: "USD",
    }).format(value);
  }

  handleBack() {
    this.dispatchEvent(new CustomEvent("back"));
  }

  handleCancel() {
    this.dispatchEvent(new CustomEvent("cancel"));
  }

  async handleSave() {
    if (!this.validateAllRows()) {
      this.dispatchError("Please fix validation errors before saving");
      return;
    }

    this.isSaving = true;

    try {
      const lineItemsData = this.lineItems.map((item) => ({
        pricebookEntryId: item.pricebookEntryId,
        listPrice: item.listPrice,
        quantity: item.quantity,
        discountValue: item.discountValue,
        serviceDate: item.serviceDate,
        description: item.description,
      }));

      await saveLineItems({
        opportunityId: this.opportunityId,
        lineItemsJson: JSON.stringify(lineItemsData),
      });

      this.dispatchEvent(
        new CustomEvent("savecomplete", {
          detail: { count: this.lineItems.length },
        })
      );
    } catch (error) {
      this.dispatchError(error.body?.message || "Failed to save line items");
    } finally {
      this.isSaving = false;
    }
  }

  dispatchError(message) {
    this.dispatchEvent(
      new CustomEvent("error", {
        detail: { message },
      })
    );
  }
}
```

**Step 2: Create the HTML template**

```html
<template>
  <div class="slds-p-around_medium">
    <!-- Header -->
    <h2 class="slds-text-heading_medium slds-m-bottom_medium">
      Edit Selected Products
    </h2>

    <!-- No Items Message -->
    <template lwc:if={!hasLineItems}>
      <div class="slds-illustration slds-illustration_small">
        <p class="slds-text-body_regular slds-text-color_weak">
          No products selected. Go back to select products.
        </p>
      </div>
    </template>

    <!-- Edit Table -->
    <template lwc:if={hasLineItems}>
      <div class="table-container">
        <table
          class="slds-table slds-table_cell-buffer slds-table_bordered slds-table_striped"
        >
          <thead>
            <tr class="slds-line-height_reset">
              <th scope="col" class="slds-text-title_caps col-num">#</th>
              <th scope="col" class="slds-text-title_caps col-product">
                <abbr title="Required">*</abbr>Product
              </th>
              <th scope="col" class="slds-text-title_caps col-quantity">
                <abbr title="Required">*</abbr>Quantity
              </th>
              <th scope="col" class="slds-text-title_caps col-discount">
                Discount
              </th>
              <th scope="col" class="slds-text-title_caps col-price">
                <abbr title="Required">*</abbr>Sales Price
              </th>
              <th scope="col" class="slds-text-title_caps col-date">Date</th>
              <th scope="col" class="slds-text-title_caps col-description">
                Description
              </th>
              <th scope="col" class="slds-text-title_caps col-action"></th>
            </tr>
          </thead>
          <tbody>
            <template for:each={lineItems} for:item="item">
              <tr key={item.key} class="slds-hint-parent">
                <!-- Row Number -->
                <td data-label="#">
                  <div class="slds-truncate">{item.rowNumber}</div>
                </td>

                <!-- Product Name (locked) -->
                <td data-label="Product">
                  <div class="slds-grid slds-grid_vertical-align-center">
                    <span class="slds-truncate" title={item.productName}>
                      {item.productName}
                    </span>
                    <lightning-icon
                      icon-name="utility:lock"
                      size="xx-small"
                      alternative-text="Locked"
                      class="slds-m-left_xx-small"
                    >
                    </lightning-icon>
                  </div>
                </td>

                <!-- Quantity -->
                <td data-label="Quantity">
                  <lightning-input
                    type="number"
                    value={item.quantity}
                    min="1"
                    step="1"
                    data-index={item.rowNumber}
                    onchange={handleQuantityChange}
                    message-when-range-underflow="Minimum is 1"
                    variant="label-hidden"
                    class="quantity-input"
                  >
                  </lightning-input>
                  <template lwc:if={item.quantityError}>
                    <div class="slds-text-color_error slds-text-body_small">
                      {item.quantityError}
                    </div>
                  </template>
                </td>

                <!-- Discount Value -->
                <td data-label="Discount">
                  <lightning-input
                    type="number"
                    value={item.discountValue}
                    min="0"
                    step="0.01"
                    formatter="currency"
                    data-index={item.rowNumber}
                    onchange={handleDiscountChange}
                    variant="label-hidden"
                    class="discount-input"
                  >
                  </lightning-input>
                  <template lwc:if={item.discountError}>
                    <div class="slds-text-color_error slds-text-body_small">
                      {item.discountError}
                    </div>
                  </template>
                </td>

                <!-- Sales Price (calculated, read-only) -->
                <td data-label="Sales Price">
                  <div class="slds-truncate sales-price">
                    <lightning-formatted-number
                      value={item.salesPrice}
                      format-style="currency"
                      currency-code="USD"
                    >
                    </lightning-formatted-number>
                  </div>
                </td>

                <!-- Service Date -->
                <td data-label="Date">
                  <lightning-input
                    type="date"
                    value={item.serviceDate}
                    data-index={item.rowNumber}
                    onchange={handleDateChange}
                    variant="label-hidden"
                    class="date-input"
                  >
                  </lightning-input>
                </td>

                <!-- Description -->
                <td data-label="Description">
                  <lightning-input
                    type="text"
                    value={item.description}
                    max-length="255"
                    data-index={item.rowNumber}
                    onchange={handleDescriptionChange}
                    variant="label-hidden"
                    class="description-input"
                  >
                  </lightning-input>
                </td>

                <!-- Delete Button -->
                <td data-label="Action">
                  <lightning-button-icon
                    icon-name="utility:delete"
                    alternative-text="Delete"
                    variant="bare"
                    data-index={item.rowNumber}
                    onclick={handleDeleteRow}
                  >
                  </lightning-button-icon>
                </td>
              </tr>
            </template>
          </tbody>
        </table>
      </div>
    </template>

    <!-- Footer Buttons -->
    <div class="slds-m-top_medium slds-grid">
      <div class="slds-col">
        <lightning-button label="Back" onclick={handleBack}> </lightning-button>
      </div>
      <div class="slds-col slds-text-align_right">
        <lightning-button
          label="Cancel"
          onclick={handleCancel}
          class="slds-m-right_x-small"
        >
        </lightning-button>
        <lightning-button
          label="Save"
          variant="brand"
          onclick={handleSave}
          disabled={isSaveDisabled}
        >
          <template lwc:if={isSaving}>
            <lightning-spinner
              alternative-text="Saving"
              size="small"
              class="slds-m-left_x-small"
            >
            </lightning-spinner>
          </template>
        </lightning-button>
      </div>
    </div>
  </div>
</template>
```

**Step 3: Create the CSS file**

```css
:host {
  display: block;
}

.table-container {
  max-height: 400px;
  overflow-y: auto;
}

.col-num {
  width: 3rem;
}

.col-product {
  width: 20%;
  min-width: 150px;
}

.col-quantity {
  width: 10%;
  min-width: 80px;
}

.col-discount {
  width: 12%;
  min-width: 100px;
}

.col-price {
  width: 12%;
  min-width: 100px;
}

.col-date {
  width: 12%;
  min-width: 120px;
}

.col-description {
  width: 20%;
  min-width: 150px;
}

.col-action {
  width: 3rem;
}

.sales-price {
  font-weight: 600;
  color: #080707;
}

.quantity-input,
.discount-input,
.date-input,
.description-input {
  width: 100%;
}

.slds-text-color_error {
  margin-top: 0.25rem;
}
```

**Step 4: Create the meta.xml file**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<LightningComponentBundle xmlns="http://soap.sforce.com/2006/04/metadata">
    <apiVersion>62.0</apiVersion>
    <isExposed>false</isExposed>
    <masterLabel>Add Products Edit</masterLabel>
    <description>Edit selected products screen for Add Products wizard</description>
</LightningComponentBundle>
```

---

## Task 6: Deploy and Test All Components

**Step 1: Deploy Apex classes**

Run:
```bash
sf project deploy start \
  --source-dir force-app/main/default/classes/AddProductsController.cls \
  --source-dir force-app/main/default/classes/AddProductsControllerTest.cls \
  --target-org <alias> \
  --wait 10
```

**Step 2: Run Apex tests and verify coverage**

Run:
```bash
sf apex run test \
  --class-names AddProductsControllerTest \
  --code-coverage \
  --result-format human \
  --wait 10 \
  --target-org <alias>
```

Expected: All tests pass, AddProductsController has 90%+ coverage

**Step 3: Deploy LWC components**

Run:
```bash
sf project deploy start \
  --source-dir force-app/main/default/lwc/addProductsToOpportunity \
  --source-dir force-app/main/default/lwc/addProductsSearch \
  --source-dir force-app/main/default/lwc/addProductsEdit \
  --target-org <alias> \
  --wait 10
```

**Step 4: Manual verification**

1. Open an Opportunity with a Price Book assigned
2. Add the component to the record page (or create a Quick Action)
3. Test the full flow:
   - Search products
   - Select multiple products
   - Click Next
   - Enter quantities and discounts
   - Verify Sales Price updates reactively
   - Verify validation errors appear for invalid discounts
   - Save and verify OpportunityLineItems created

---

## Task 7: Code Validation with Codex

**Step 1: Validate all code**

Use the codex-validation skill to review:
- Apex controller security (SOQL injection, FLS)
- LWC best practices
- Error handling completeness
- Test coverage gaps

**Step 2: Address any findings**

Fix issues identified by Codex validation before final commit.

---

## Commit Checkpoints

| After Task | Commit Message |
|------------|----------------|
| Task 2 | `feat(apex): add AddProductsController with tests` |
| Task 5 | `feat(lwc): add Add Products wizard components` |
| Task 7 | `fix: address code review findings` |
