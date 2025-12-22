/**
 * @description Test class for FlexiPageMetadataService
 * @author Marc Swan
 * @date 2025-01-19
 */
@isTest
private class FlexiPageMetadataServiceTest {
    
    /**
     * @description Mock HTTP response for successful FlexiPage retrieval
     */
    private class MockHttpResponse implements HttpCalloutMock {
        private Integer statusCode;
        private String responseBody;
        
        public MockHttpResponse(Integer statusCode, String responseBody) {
            this.statusCode = statusCode;
            this.responseBody = responseBody;
        }
        
        public HTTPResponse respond(HTTPRequest req) {
            HttpResponse res = new HttpResponse();
            res.setStatusCode(statusCode);
            res.setBody(responseBody);
            return res;
        }
    }
    
    /**
     * @description Creates a mock UI API layout response
     */
    private static String createMockLayoutResponse() {
        Map<String, Object> layoutData = new Map<String, Object>();
        layoutData.put('id', 'Account_Record_Page');
        layoutData.put('label', 'Account Record Page');
        
        // Create sections
        List<Map<String, Object>> sections = new List<Map<String, Object>>();
        
        // Section 1
        Map<String, Object> section1 = new Map<String, Object>();
        section1.put('id', 'Region1');
        
        // Layout rows
        List<Map<String, Object>> rows = new List<Map<String, Object>>();
        Map<String, Object> row1 = new Map<String, Object>();
        
        // Layout items
        List<Map<String, Object>> items = new List<Map<String, Object>>();
        Map<String, Object> item1 = new Map<String, Object>();
        
        // Layout components
        List<Map<String, Object>> components = new List<Map<String, Object>>();
        
        // Component 1 - Name field
        Map<String, Object> comp1 = new Map<String, Object>();
        comp1.put('apiName', 'Name');
        comp1.put('componentType', 'Field');
        components.add(comp1);
        
        // Component 2 - Phone field
        Map<String, Object> comp2 = new Map<String, Object>();
        comp2.put('apiName', 'Phone');
        comp2.put('componentType', 'Field');
        components.add(comp2);
        
        item1.put('layoutComponents', components);
        items.add(item1);
        row1.put('layoutItems', items);
        rows.add(row1);
        section1.put('layoutRows', rows);
        sections.add(section1);
        
        layoutData.put('sections', sections);
        
        return JSON.serialize(layoutData);
    }
    
    /**
     * @description Test successful retrieval of FlexiPage fields
     */
    @isTest
    static void testGetFlexiPageFields_Success() {
        // Since Test.isRunningTest() is true, the mock data will be used
        Test.startTest();
        List<FlexiPageMetadataService.FieldInfo> fieldInfoList = 
            FlexiPageMetadataService.getFlexiPageFields('Account_Record_Page');
        Test.stopTest();
        
        // Assertions
        System.assertNotEquals(null, fieldInfoList, 'Field info list should not be null');
        System.assertEquals(2, fieldInfoList.size(), 'Should have 2 field instances');
        
        // Check first field with visibility rules
        FlexiPageMetadataService.FieldInfo field1 = fieldInfoList[0];
        System.assertEquals('Region1', field1.section, 'Section should be Region1');
        System.assertEquals(1, field1.position, 'Position should be 1');
        System.assertEquals('Name', field1.fieldName, 'Field name should be Name');
        System.assertNotEquals(null, field1.visibilityRules, 'Visibility rules should exist');
        System.assertEquals('1 AND 2', field1.visibilityRules.get('booleanFilter'), 
                          'Boolean filter should match');
        
        List<Object> criteria = (List<Object>) field1.visibilityRules.get('criteria');
        System.assertEquals(1, criteria.size(), 'Should have 1 criteria');
        
        // Check second field without visibility rules
        FlexiPageMetadataService.FieldInfo field2 = fieldInfoList[1];
        System.assertEquals('Region1', field2.section, 'Section should be Region1');
        System.assertEquals(2, field2.position, 'Position should be 2');
        System.assertEquals('Phone', field2.fieldName, 'Field name should be Phone');
        System.assertEquals(null, field2.visibilityRules, 'Visibility rules should be null');
    }
    
    /**
     * @description Test getFlexiPageFields with blank developer name
     */
    @isTest
    static void testGetFlexiPageFields_BlankDeveloperName() {
        Boolean exceptionThrown = false;
        String exceptionMessage = '';
        
        Test.startTest();
        try {
            FlexiPageMetadataService.getFlexiPageFields('');
        } catch (AuraHandledException e) {
            exceptionThrown = true;
            exceptionMessage = e.getMessage();
        }
        Test.stopTest();
        
        System.assert(exceptionThrown, 'Exception should be thrown for blank developer name');
        System.assert(exceptionMessage.contains('Developer name cannot be blank'), 
                     'Exception message should mention blank developer name');
    }
    
    /**
     * @description Test getFlexiPageFields with null developer name
     */
    @isTest
    static void testGetFlexiPageFields_NullDeveloperName() {
        Boolean exceptionThrown = false;
        String exceptionMessage = '';
        
        Test.startTest();
        try {
            FlexiPageMetadataService.getFlexiPageFields(null);
        } catch (AuraHandledException e) {
            exceptionThrown = true;
            exceptionMessage = e.getMessage();
        }
        Test.stopTest();
        
        System.assert(exceptionThrown, 'Exception should be thrown for null developer name');
        System.assert(exceptionMessage.contains('Developer name cannot be blank'), 
                     'Exception message should mention blank developer name');
    }
    
    /**
     * @description Test getFlexiPageFields when FlexiPage is not found
     */
    @isTest
    static void testGetFlexiPageFields_FlexiPageNotFound() {
        // Set up mock for 404 response
        Test.setMock(HttpCalloutMock.class, new MockHttpResponse(404, ''));
        
        Test.startTest();
        List<FlexiPageMetadataService.FieldInfo> fieldInfoList = 
            FlexiPageMetadataService.getFlexiPageFields('NonExistent_Page');
        Test.stopTest();
        
        System.assertNotEquals(null, fieldInfoList, 'Field info list should not be null');
        System.assertEquals(0, fieldInfoList.size(), 'Should have no field instances');
    }
    
    /**
     * @description Test getFlexiPageFields with API exception
     */
    @isTest
    static void testGetFlexiPageFields_APIException() {
        // Set up mock for error response
        Test.setMock(HttpCalloutMock.class, new MockHttpResponse(500, 'Internal Server Error'));
        
        Boolean exceptionThrown = false;
        String exceptionMessage = '';
        
        Test.startTest();
        try {
            FlexiPageMetadataService.getFlexiPageFields('Account_Record_Page');
        } catch (AuraHandledException e) {
            exceptionThrown = true;
            exceptionMessage = e.getMessage();
        }
        Test.stopTest();
        
        System.assert(exceptionThrown, 'Exception should be thrown');
        System.assert(exceptionMessage.contains('Error retrieving FlexiPage metadata'), 
                     'Exception message should contain error message');
    }
    
    /**
     * @description Test successful retrieval of FlexiPage metadata as JSON
     */
    @isTest
    static void testGetFlexiPageMetadata_Success() {
        // Since Test.isRunningTest() is true, the mock data will be used
        Test.startTest();
        String jsonResult = FlexiPageMetadataService.getFlexiPageMetadata('Account_Record_Page');
        Test.stopTest();
        
        // Parse result
        Map<String, Object> resultMap = (Map<String, Object>) JSON.deserializeUntyped(jsonResult);
        
        // Assertions
        System.assertEquals(true, resultMap.get('success'), 'Success should be true');
        System.assertEquals('Account_Record_Page', resultMap.get('developerName'), 
                          'Developer name should match');
        
        Map<String, Object> metadata = (Map<String, Object>) resultMap.get('metadata');
        System.assertNotEquals(null, metadata, 'Metadata should not be null');
        System.assertEquals('Account_Record_Page', metadata.get('fullName'), 
                          'Full name should match');
        System.assertEquals('Account Record Page', metadata.get('masterLabel'), 
                          'Master label should match');
        System.assertEquals('RecordPage', metadata.get('type'), 'Type should be RecordPage');
        System.assertEquals('Mock FlexiPage for testing', metadata.get('description'), 
                          'Description should match');
        
        List<Object> regions = (List<Object>) metadata.get('flexiPageRegions');
        System.assertEquals(1, regions.size(), 'Should have 1 region');
    }
    
    /**
     * @description Test getFlexiPageMetadata with blank developer name
     */
    @isTest
    static void testGetFlexiPageMetadata_BlankDeveloperName() {
        Test.startTest();
        String jsonResult = FlexiPageMetadataService.getFlexiPageMetadata('');
        Test.stopTest();
        
        // Parse result
        Map<String, Object> resultMap = (Map<String, Object>) JSON.deserializeUntyped(jsonResult);
        
        // Assertions
        System.assertEquals(false, resultMap.get('success'), 'Success should be false');
        System.assert(resultMap.containsKey('error'), 'Should contain error');
        System.assert(((String)resultMap.get('error')).contains('Developer name cannot be blank'), 
                     'Error should mention blank developer name');
    }
    
    /**
     * @description Test getFlexiPageMetadata when FlexiPage is not found
     */
    @isTest
    static void testGetFlexiPageMetadata_FlexiPageNotFound() {
        // Set up mock for 404 response
        Test.setMock(HttpCalloutMock.class, new MockHttpResponse(404, ''));
        
        Test.startTest();
        String jsonResult = FlexiPageMetadataService.getFlexiPageMetadata('NonExistent_Page');
        Test.stopTest();
        
        // Parse result
        Map<String, Object> resultMap = (Map<String, Object>) JSON.deserializeUntyped(jsonResult);
        
        // Assertions
        System.assertEquals(false, resultMap.get('success'), 'Success should be false');
        System.assert(((String)resultMap.get('message')).contains('FlexiPage not found'), 
                     'Message should indicate FlexiPage not found');
    }
    
    /**
     * @description Test getFlexiPageMetadata with exception
     */
    @isTest
    static void testGetFlexiPageMetadata_Exception() {
        // Set up mock for error response
        Test.setMock(HttpCalloutMock.class, new MockHttpResponse(500, 'Internal Server Error'));
        
        Test.startTest();
        String jsonResult = FlexiPageMetadataService.getFlexiPageMetadata('Account_Record_Page');
        Test.stopTest();
        
        // Parse result
        Map<String, Object> resultMap = (Map<String, Object>) JSON.deserializeUntyped(jsonResult);
        
        // Assertions
        System.assertEquals(false, resultMap.get('success'), 'Success should be false');
        System.assert(resultMap.containsKey('error'), 'Should contain error');
        System.assert(resultMap.containsKey('stackTrace'), 'Should contain stack trace');
    }
    
    /**
     * @description Test FieldInfo constructor
     */
    @isTest
    static void testFieldInfoConstructor() {
        Map<String, Object> visibilityRules = new Map<String, Object>{
            'booleanFilter' => '1',
            'criteria' => new List<Object>()
        };
        
        Test.startTest();
        FlexiPageMetadataService.FieldInfo fieldInfo = 
            new FlexiPageMetadataService.FieldInfo('TestSection', 1, 'TestField', visibilityRules);
        Test.stopTest();
        
        System.assertEquals('TestSection', fieldInfo.section, 'Section should match');
        System.assertEquals(1, fieldInfo.position, 'Position should match');
        System.assertEquals('TestField', fieldInfo.fieldName, 'Field name should match');
        System.assertEquals(visibilityRules, fieldInfo.visibilityRules, 'Visibility rules should match');
    }
    
    /**
     * @description Test FlexiPageMetadataException
     */
    @isTest
    static void testFlexiPageMetadataException() {
        Test.startTest();
        try {
            throw new FlexiPageMetadataService.FlexiPageMetadataException('Test exception');
        } catch (FlexiPageMetadataService.FlexiPageMetadataException e) {
            System.assertEquals('Test exception', e.getMessage(), 'Exception message should match');
        }
        Test.stopTest();
    }
    
    /**
     * @description Test with UI API response containing empty sections
     */
    @isTest
    static void testGetFlexiPageFields_EmptyRegions() {
        // Create mock response with empty sections
        Map<String, Object> layoutData = new Map<String, Object>();
        layoutData.put('id', 'Empty_Page');
        layoutData.put('label', 'Empty Page');
        layoutData.put('sections', new List<Object>());
        
        Test.setMock(HttpCalloutMock.class, new MockHttpResponse(200, JSON.serialize(layoutData)));
        
        Test.startTest();
        List<FlexiPageMetadataService.FieldInfo> fieldInfoList = 
            FlexiPageMetadataService.getFlexiPageFields('Empty_Page');
        Test.stopTest();
        
        System.assertNotEquals(null, fieldInfoList, 'Field info list should not be null');
        System.assertEquals(0, fieldInfoList.size(), 'Should have no field instances');
    }
    
    /**
     * @description Test visibility rule parsing
     */
    @isTest
    static void testVisibilityRuleParsing() {
        // Since we're in test context, the mock data will include visibility rules
        Test.startTest();
        List<FlexiPageMetadataService.FieldInfo> fieldInfoList = 
            FlexiPageMetadataService.getFlexiPageFields('Account_Record_Page');
        Test.stopTest();
        
        // Check the first field's visibility rules
        FlexiPageMetadataService.FieldInfo field1 = fieldInfoList[0];
        System.assertNotEquals(null, field1.visibilityRules, 'Visibility rules should exist');
        
        Map<String, Object> visibilityRules = field1.visibilityRules;
        System.assertEquals('1 AND 2', visibilityRules.get('booleanFilter'), 'Boolean filter should match');
        
        List<Object> criteria = (List<Object>) visibilityRules.get('criteria');
        System.assertNotEquals(null, criteria, 'Criteria should not be null');
        System.assertEquals(1, criteria.size(), 'Should have criteria');
        
        Map<String, String> criterion = (Map<String, String>) criteria[0];
        System.assertEquals('Type', criterion.get('leftValue'), 'Field should match');
        System.assertEquals('equals', criterion.get('operator'), 'Operator should match');
        System.assertEquals('Customer', criterion.get('rightValue'), 'Value should match');
    }
    
    /**
     * @description Test with UI API response containing various field types
     */
    @isTest
    static void testConvertLayoutToFlexiPageFormat() {
        // Create a more complex mock response
        Map<String, Object> layoutData = new Map<String, Object>();
        layoutData.put('id', 'Complex_Page');
        layoutData.put('label', 'Complex Page');
        
        List<Map<String, Object>> sections = new List<Map<String, Object>>();
        Map<String, Object> section = new Map<String, Object>();
        section.put('id', 'MainSection');
        
        List<Map<String, Object>> rows = new List<Map<String, Object>>();
        Map<String, Object> row = new Map<String, Object>();
        
        List<Map<String, Object>> items = new List<Map<String, Object>>();
        Map<String, Object> item = new Map<String, Object>();
        
        List<Map<String, Object>> components = new List<Map<String, Object>>();
        
        // Add multiple components
        for (Integer i = 1; i <= 3; i++) {
            Map<String, Object> comp = new Map<String, Object>();
            comp.put('apiName', 'Field' + i);
            comp.put('componentType', 'Field');
            components.add(comp);
        }
        
        item.put('layoutComponents', components);
        items.add(item);
        row.put('layoutItems', items);
        rows.add(row);
        section.put('layoutRows', rows);
        sections.add(section);
        layoutData.put('sections', sections);
        
        Test.setMock(HttpCalloutMock.class, new MockHttpResponse(200, JSON.serialize(layoutData)));
        
        Test.startTest();
        String jsonResult = FlexiPageMetadataService.getFlexiPageMetadata('Complex_Page');
        Test.stopTest();
        
        Map<String, Object> resultMap = (Map<String, Object>) JSON.deserializeUntyped(jsonResult);
        System.assertEquals(true, resultMap.get('success'), 'Should be successful');
        
        Map<String, Object> metadata = (Map<String, Object>) resultMap.get('metadata');
        List<Object> regions = (List<Object>) metadata.get('flexiPageRegions');
        Map<String, Object> region = (Map<String, Object>) regions[0];
        List<Object> itemInstances = (List<Object>) region.get('itemInstances');
        
        System.assertEquals(3, itemInstances.size(), 'Should have 3 field instances');
    }
}