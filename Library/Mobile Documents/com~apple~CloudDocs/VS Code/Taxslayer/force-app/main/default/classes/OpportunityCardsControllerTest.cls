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
