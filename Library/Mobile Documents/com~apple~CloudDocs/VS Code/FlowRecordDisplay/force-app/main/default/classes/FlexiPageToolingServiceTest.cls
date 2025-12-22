@isTest
private class FlexiPageToolingServiceTest {
    private static final String TEST_FLEXIPAGE_NAME = 'Account_Record_Page';
    private static final String TEST_OBJECT_API_NAME = 'Account';
    private static final String TEST_RECORD_ID = '001xx000003DGb0AAG';

    @isTest
    static void testGetFlexiPageMetadata_Success() {
        Test.setMock(HttpCalloutMock.class, new FlexiPageToolingServiceMock(false));

        Test.startTest();
        String result = FlexiPageToolingService.getFlexiPageMetadata(TEST_FLEXIPAGE_NAME);
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        Map<String, Object> resultMap = (Map<String, Object>) JSON.deserializeUntyped(result);
        System.assertEquals('FlexiPage', resultMap.get('type'), 'Result should contain FlexiPage metadata');
    }

    @isTest
    static void testGetFlexiPageMetadata_NoRecordsFound() {
        Test.setMock(HttpCalloutMock.class, new FlexiPageToolingServiceMock(true));

        Test.startTest();
        try {
            FlexiPageToolingService.getFlexiPageMetadata(TEST_FLEXIPAGE_NAME);
            System.assert(false, 'Exception should have been thrown');
        } catch (AuraHandledException e) {
            System.assertEquals('Script-thrown exception', e.getMessage(), 'Incorrect exception message');
        }
        Test.stopTest();
    }

    @isTest
    static void testGetFieldValues_Success() {
        Account testAccount = new Account(
                Name = 'Test Account',
                Type = 'Customer',
                Industry = 'Technology',
                AnnualRevenue = 1000000
        );
        insert testAccount;

        // Query the record back to verify the Name field
        Account insertedAccount = [SELECT Id, Name FROM Account WHERE Id = :testAccount.Id];
        System.debug('Inserted Account Name: ' + insertedAccount.Name);

        Test.startTest();
        List<String> fieldApiNames = new List<String>{'Name', 'AnnualRevenue'};
        Map<String, Object> result = FlexiPageToolingService.getFieldValues(testAccount.Id, TEST_OBJECT_API_NAME, fieldApiNames);
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.debug('Result: ' + result);

        // Check structure of new response format
        System.assert(result.containsKey('values'), 'Result should contain values map');
        System.assert(result.containsKey('metadata'), 'Result should contain metadata map');
        
        Map<String, Object> values = (Map<String, Object>)result.get('values');
        if (values.containsKey('name')) {
            System.assertEquals('Test Account', (String)values.get('name'), 'Name field should match');
        } else {
            System.assert(false, 'Name field is missing from the values');
        }
    }

    @isTest
    static void testGetFieldValues_InvalidObject() {
        Test.startTest();
        try {
            FlexiPageToolingService.getFieldValues(TEST_RECORD_ID, 'InvalidObject', new List<String>());
            System.assert(false, 'Exception should have been thrown');
        } catch (AuraHandledException e) {
            System.assertEquals('Script-thrown exception', e.getMessage(), 'Incorrect exception message');
        }
        Test.stopTest();
    }

    private class FlexiPageToolingServiceMock implements HttpCalloutMock {
        private Boolean returnEmptyResults;

        public FlexiPageToolingServiceMock(Boolean returnEmptyResults) {
            this.returnEmptyResults = returnEmptyResults;
        }

        public HTTPResponse respond(HTTPRequest req) {
            HttpResponse res = new HttpResponse();
            res.setHeader('Content-Type', 'application/json');
            res.setStatusCode(200);

            if (returnEmptyResults) {
                res.setBody('{"records":[]}');
            } else {
                res.setBody('{"records":[{"Metadata":{"type":"FlexiPage","fullName":"' + TEST_FLEXIPAGE_NAME + '"}}]}');
            }

            return res;
        }
    }
}