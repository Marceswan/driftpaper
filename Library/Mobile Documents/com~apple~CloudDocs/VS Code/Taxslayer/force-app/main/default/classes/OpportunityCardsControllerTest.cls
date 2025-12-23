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
        // Query has LIMIT of (15 * 2 + 10) = 40, so we get at most 40 total records
        // 5 open opps from setup + 200 new = 205 total, but query returns max 40
        System.assert(result.totalOpenCount >= 15, 'Should have at least 15 open opportunities');
        System.assertEquals(15, result.openOpportunities.size(), 'Should only return 15 based on limitCount');

        // Verify the controller respects the limit parameter
        System.assert(result.openOpportunities.size() <= 15, 'Should not exceed requested limit');
    }
}
