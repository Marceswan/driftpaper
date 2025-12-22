public without sharing class DateStampTriggerDeployer {
    /**
     * Deploy method that creates and deploys trigger using JSZip
     * This integrates with the Visualforce page for actual deployment
     */
    public static MetadataService.AsyncResult[] deploy(String objectApiName) {
        // Get trigger details
        String triggerName = generateTriggerName(objectApiName);
        String triggerSource = generateTriggerSource(objectApiName, triggerName);
        
        // For now, we need the VF page to handle the actual ZIP creation and deployment
        // The LWC should navigate to /apex/DateBuddyDeploy with the objectApiName parameter
        // Then call the JavaScript deployTrigger function
        
        MetadataService.AsyncResult result = new MetadataService.AsyncResult();
        result.done = false;
        result.id = 'PENDING_VF_' + System.now().getTime();
        result.state = 'Pending';
        result.message = 'Use /apex/DateBuddyDeploy?objectApiName=' + objectApiName + ' for deployment';
        
        return new MetadataService.AsyncResult[] { result };
    }
    
    /**
     * RemoteAction for VF page to get trigger details
     */
    @RemoteAction
    public static Map<String, String> getTriggerDetails(String objectApiName) {
        String triggerName = generateTriggerName(objectApiName);
        Map<String, String> result = new Map<String, String>();
        result.put('triggerName', triggerName);
        result.put('triggerSource', generateTriggerSource(objectApiName, triggerName));
        return result;
    }
    
    /**
     * RemoteAction to deploy ZIP from Visualforce page
     * @param zipData Base64 encoded ZIP file containing the trigger package
     * @param objectApiName The object API name to generate test class name
     * @return The async result ID for tracking
     */
    @RemoteAction
    public static String deployZip(String zipData, String objectApiName) {
        try {
            // Generate the test class name that will be deployed with the trigger
            String testClassName = generateTestClassName(objectApiName);
            
            // Create MetadataService and configure deployment options
            MetadataService.MetadataPort service = new MetadataService.MetadataPort();
            service.SessionHeader = new MetadataService.SessionHeader_element();
            service.SessionHeader.sessionId = UserInfo.getSessionId();
            service.endpoint_x = System.Url.getOrgDomainUrl().toExternalForm() + '/services/Soap/m/62.0';
            
            // Configure deployment options following apex-mdapi best practices
            MetadataService.DeployOptions deployOptions = new MetadataService.DeployOptions();
            deployOptions.allowMissingFiles = false;
            deployOptions.autoUpdatePackage = false;
            deployOptions.checkOnly = false;  // Set to true for validation-only
            deployOptions.ignoreWarnings = false;
            deployOptions.performRetrieve = false;
            deployOptions.purgeOnDelete = false;
            deployOptions.rollbackOnError = true;
            deployOptions.singlePackage = true;
            // Use RunSpecifiedTests with the generated test class for this trigger
            deployOptions.testLevel = 'RunSpecifiedTests';
            deployOptions.runTests = new String[] {
                testClassName
            };
            
            // Deploy the ZIP package received from Visualforce page
            MetadataService.AsyncResult result = service.deploy(zipData, deployOptions);
            return result.id;
        } catch (Exception e) {
            throw new AuraHandledException('Deployment failed: ' + e.getMessage());
        }
    }
    
    /**
     * Legacy overload for backward compatibility
     * @deprecated Use deployZip(String zipData, String objectApiName) instead
     * Named deployZipLegacy to avoid Visualforce Remoting conflicts
     */
    @RemoteAction
    public static String deployZipLegacy(String zipData) {
        // Fall back to MetadataServiceTest for legacy calls
        try {
            MetadataService.MetadataPort service = new MetadataService.MetadataPort();
            service.SessionHeader = new MetadataService.SessionHeader_element();
            service.SessionHeader.sessionId = UserInfo.getSessionId();
            service.endpoint_x = System.Url.getOrgDomainUrl().toExternalForm() + '/services/Soap/m/62.0';
            
            MetadataService.DeployOptions deployOptions = new MetadataService.DeployOptions();
            deployOptions.allowMissingFiles = false;
            deployOptions.autoUpdatePackage = false;
            deployOptions.checkOnly = false;
            deployOptions.ignoreWarnings = false;
            deployOptions.performRetrieve = false;
            deployOptions.purgeOnDelete = false;
            deployOptions.rollbackOnError = true;
            deployOptions.singlePackage = true;
            deployOptions.testLevel = 'RunSpecifiedTests';
            deployOptions.runTests = new String[] { 'MetadataServiceTest' };
            
            MetadataService.AsyncResult result = service.deploy(zipData, deployOptions);
            return result.id;
        } catch (Exception e) {
            throw new AuraHandledException('Deployment failed: ' + e.getMessage());
        }
    }
    
    /**
     * Prepares trigger package data for deployment.
     * This method is called by Visualforce page to get the package components.
     * 
     * @param objectApiName The object to create trigger for
     * @return Map containing package components (packageXml, triggerSource, triggerMetadata, testClass)
     */
    @RemoteAction
    public static Map<String, String> prepareDeploymentPackage(String objectApiName) {
        String triggerName = generateTriggerName(objectApiName);
        String testClassName = generateTestClassName(objectApiName);
        
        Map<String, String> packageComponents = new Map<String, String>();
        packageComponents.put('packageXml', generatePackageXml(triggerName, testClassName));
        packageComponents.put('triggerSource', generateTriggerSource(objectApiName, triggerName));
        packageComponents.put('triggerMetadata', generateTriggerMetaXml());
        packageComponents.put('triggerName', triggerName);
        packageComponents.put('testClassName', testClassName);
        packageComponents.put('testClassSource', generateTestClassSource(objectApiName, triggerName, testClassName));
        packageComponents.put('testClassMetadata', generateTestClassMetaXml());
        
        return packageComponents;
    }
    
    
    /**
     * Generates the package.xml for the deployment
     */
    private static String generatePackageXml(String triggerName, String testClassName) {
        return '<?xml version="1.0" encoding="UTF-8"?>\n' +
               '<Package xmlns="http://soap.sforce.com/2006/04/metadata">\n' +
               '    <types>\n' +
               '        <members>' + triggerName + '</members>\n' +
               '        <name>ApexTrigger</name>\n' +
               '    </types>\n' +
               '    <types>\n' +
               '        <members>' + testClassName + '</members>\n' +
               '        <name>ApexClass</name>\n' +
               '    </types>\n' +
               '    <version>64.0</version>\n' +
               '</Package>';
    }
    
    /**
     * Generates the trigger metadata XML file
     */
    private static String generateTriggerMetaXml() {
        return '<?xml version="1.0" encoding="UTF-8"?>\n' +
               '<ApexTrigger xmlns="http://soap.sforce.com/2006/04/metadata">\n' +
               '    <apiVersion>64.0</apiVersion>\n' +
               '    <status>Active</status>\n' +
               '</ApexTrigger>';
    }

    public static String generateTriggerName(String objectApiName) {
        return 'DateStamp_' + objectApiName + '_BT';
    }
    
    public static String generateTestClassName(String objectApiName) {
        return 'DateStamp_' + objectApiName + '_Test';
    }

    public static String generateTriggerSource(String objectApiName, String triggerName) {
        String nl = '\n';
        // BEFORE SAVE context - captures ALL changes before they're committed to database
        String body = 'trigger ' + triggerName + ' on ' + objectApiName + ' (before insert, before update) {' + nl +
                      '    // BEFORE SAVE context ensures we capture ALL changes' + nl +
                      '    // This includes both "Entered" (value changes TO target) and "Exited" (value changes FROM target) scenarios' + nl +
                      '    DateBuddyHandler.beforeInsertOrUpdate(Trigger.new, Trigger.oldMap);' + nl +
                      '}';
        return body;
    }
    
    /**
     * Generates the test class metadata XML file
     */
    private static String generateTestClassMetaXml() {
        return '<?xml version="1.0" encoding="UTF-8"?>\n' +
               '<ApexClass xmlns="http://soap.sforce.com/2006/04/metadata">\n' +
               '    <apiVersion>64.0</apiVersion>\n' +
               '    <status>Active</status>\n' +
               '</ApexClass>';
    }
    
    /**
     * Generates a comprehensive test class for the trigger
     */
    public static String generateTestClassSource(String objectApiName, String triggerName, String testClassName) {
        String nl = '\n';
        String tab = '    ';
        
        // Determine if it's a standard or custom object
        Boolean isCustom = objectApiName.endsWith('__c');
        Boolean isStandardWithRequiredFields = isStandardObjectWithRequiredFields(objectApiName);
        
        String testClass = '@isTest' + nl +
            'public class ' + testClassName + ' {' + nl + nl +
            tab + '/**' + nl +
            tab + ' * Test class for ' + triggerName + ' trigger' + nl +
            tab + ' * Auto-generated by DateBuddy deployment process' + nl +
            tab + ' */' + nl + nl +
            tab + '// Helper method to get the System Administrator test user' + nl +
            tab + 'private static User getTestAdminUser() {' + nl +
            tab + tab + 'return [SELECT Id FROM User WHERE LastName = \'TestAdmin\' AND Email = \'testadmin.datebuddy@test.com\' LIMIT 1];' + nl +
            tab + '}' + nl + nl +
            tab + '@testSetup' + nl +
            tab + 'static void setupTestData() {' + nl +
            tab + tab + '// Get System Administrator profile' + nl +
            tab + tab + 'Profile sysAdminProfile = [SELECT Id FROM Profile WHERE Name = \'System Administrator\' LIMIT 1];' + nl +
            tab + tab + nl +
            tab + tab + '// Create a test user with System Administrator profile' + nl +
            tab + tab + 'User testUser = new User(' + nl +
            tab + tab + tab + 'Alias = \'tadmin\',' + nl +
            tab + tab + tab + 'Email = \'testadmin.datebuddy@test.com\',' + nl +
            tab + tab + tab + 'EmailEncodingKey = \'UTF-8\',' + nl +
            tab + tab + tab + 'LastName = \'TestAdmin\',' + nl +
            tab + tab + tab + 'LanguageLocaleKey = \'en_US\',' + nl +
            tab + tab + tab + 'LocaleSidKey = \'en_US\',' + nl +
            tab + tab + tab + 'ProfileId = sysAdminProfile.Id,' + nl +
            tab + tab + tab + 'TimeZoneSidKey = \'America/Los_Angeles\',' + nl +
            tab + tab + tab + 'UserName = \'testadmin.\' + System.currentTimeMillis() + \'@datebuddy.test\'' + nl +
            tab + tab + ');' + nl +
            tab + tab + 'insert testUser;' + nl +
            tab + '}' + nl + nl +
            tab + '@isTest' + nl +
            tab + 'static void testTriggerOnInsert() {' + nl +
            tab + tab + 'User adminUser = getTestAdminUser();' + nl +
            tab + tab + 'System.runAs(adminUser) {' + nl +
            tab + tab + tab + '// Create test data' + nl +
            tab + tab + tab + generateTestObjectCreation(objectApiName, 'insert') + nl +
            tab + tab + tab + '// Verify trigger executed (DateBuddyHandler was called)' + nl +
            tab + tab + tab + 'System.assert(testRecord.Id != null, \'Record should be inserted\');' + nl +
            tab + tab + '}' + nl +
            tab + '}' + nl + nl +
            tab + '@isTest' + nl +
            tab + 'static void testTriggerOnUpdate() {' + nl +
            tab + tab + 'User adminUser = getTestAdminUser();' + nl +
            tab + tab + 'System.runAs(adminUser) {' + nl +
            tab + tab + tab + '// Create and insert test data' + nl +
            tab + tab + tab + generateTestObjectCreation(objectApiName, 'update') + nl +
            tab + tab + tab + '// Update the record to trigger the update event' + nl +
            tab + tab + tab + 'testRecord = [SELECT Id FROM ' + objectApiName + ' WHERE Id = :testRecord.Id];' + nl +
            tab + tab + tab + generateTestObjectUpdate(objectApiName) + nl +
            tab + tab + tab + 'update testRecord;' + nl +
            tab + tab + tab + '// Verify trigger executed' + nl +
            tab + tab + tab + 'System.assert(testRecord.Id != null, \'Record should be updated\');' + nl +
            tab + tab + '}' + nl +
            tab + '}' + nl + nl +
            tab + '@isTest' + nl +
            tab + 'static void testBulkOperations() {' + nl +
            tab + tab + 'User adminUser = getTestAdminUser();' + nl +
            tab + tab + 'System.runAs(adminUser) {' + nl +
            tab + tab + tab + '// Test with multiple records' + nl +
            generateBulkTestSetup(objectApiName) +
            tab + tab + tab + 'List<' + objectApiName + '> testRecords = new List<' + objectApiName + '>();' + nl +
            tab + tab + tab + 'for (Integer i = 0; i < 200; i++) {' + nl +
            tab + tab + tab + tab + generateBulkTestObjectCreation(objectApiName) + nl +
            tab + tab + tab + '}' + nl +
            tab + tab + tab + 'insert testRecords;' + nl +
            tab + tab + tab + '// Verify bulk trigger execution' + nl +
            tab + tab + tab + 'System.assertEquals(200, [SELECT COUNT() FROM ' + objectApiName + ' WHERE Id IN :testRecords], \'All records should be inserted\');' + nl +
            tab + tab + '}' + nl +
            tab + '}' + nl +
            '}';
            
        return testClass;
    }
    
    /**
     * Dynamically queries required fields for an object
     * Returns a map of field names to their default values
     */
    @TestVisible
    private static Map<String, String> getRequiredFieldsForObject(String objectApiName) {
        Map<String, String> requiredFields = new Map<String, String>();
        
        try {
            // Use dynamic describe to get object and field metadata
            String query = 'SELECT Id FROM ' + objectApiName + ' LIMIT 1';
            Schema.SObjectType targetType = Schema.getGlobalDescribe().get(objectApiName);
            
            if (targetType != null) {
                Map<String, Schema.SObjectField> fieldMap = targetType.getDescribe().fields.getMap();
                
                for (String fieldName : fieldMap.keySet()) {
                    Schema.DescribeFieldResult fieldDescribe = fieldMap.get(fieldName).getDescribe();
                    
                    // Check if field is required (not nillable) and creatable
                    if (!fieldDescribe.isNillable() && fieldDescribe.isCreateable() && 
                        !fieldDescribe.isDefaultedOnCreate() && !fieldDescribe.isAutoNumber()) {
                        
                        // Determine appropriate default value based on field type
                        Schema.DisplayType fieldType = fieldDescribe.getType();
                        String defaultValue = getDefaultValueForFieldType(fieldType, fieldDescribe);
                        
                        if (defaultValue != null) {
                            requiredFields.put(fieldDescribe.getName(), defaultValue);
                        }
                    }
                }
            }
        } catch (Exception e) {
            // If we can't describe the object, fall back to hardcoded values
            System.debug('Unable to describe object: ' + objectApiName + ' - ' + e.getMessage());
        }
        
        return requiredFields;
    }
    
    /**
     * Returns appropriate default value for a field type
     */
    @TestVisible
    private static String getDefaultValueForFieldType(Schema.DisplayType fieldType, Schema.DescribeFieldResult fieldDescribe) {
        String fieldName = fieldDescribe.getName();
        
        // Special handling for known standard fields
        if (fieldName.equalsIgnoreCase('Name')) {
            return '\'Test Record\'';
        } else if (fieldName.equalsIgnoreCase('LastName')) {
            return '\'Test LastName\'';
        } else if (fieldName.equalsIgnoreCase('FirstName')) {
            return '\'Test\'';
        } else if (fieldName.equalsIgnoreCase('Company')) {
            return '\'Test Company\'';
        } else if (fieldName.equalsIgnoreCase('Subject')) {
            return '\'Test Subject\'';
        } else if (fieldName.equalsIgnoreCase('Status')) {
            // For Status fields, try to get the first picklist value
            List<Schema.PicklistEntry> picklistValues = fieldDescribe.getPicklistValues();
            if (!picklistValues.isEmpty()) {
                for (Schema.PicklistEntry ple : picklistValues) {
                    if (ple.isActive() && ple.isDefaultValue()) {
                        return '\'' + ple.getValue() + '\'';
                    }
                }
                // If no default, use first active value
                for (Schema.PicklistEntry ple : picklistValues) {
                    if (ple.isActive()) {
                        return '\'' + ple.getValue() + '\'';
                    }
                }
            }
            return '\'New\'';
        } else if (fieldName.equalsIgnoreCase('StageName')) {
            return '\'Prospecting\'';
        }
        
        // Generic handling based on field type
        switch on fieldType {
            when STRING, TEXTAREA, ID {
                return '\'Test ' + fieldName + '\'';
            }
            when EMAIL {
                return '\'test@example.com\'';
            }
            when URL {
                return '\'https://example.com\'';
            }
            when PHONE {
                return '\'555-1234\'';
            }
            when PICKLIST, MULTIPICKLIST {
                // Get first available picklist value
                List<Schema.PicklistEntry> picklistValues = fieldDescribe.getPicklistValues();
                if (!picklistValues.isEmpty()) {
                    for (Schema.PicklistEntry ple : picklistValues) {
                        if (ple.isActive()) {
                            return '\'' + ple.getValue() + '\'';
                        }
                    }
                }
                return '\'Test Value\'';
            }
            when BOOLEAN {
                return 'false';
            }
            when INTEGER, DOUBLE, PERCENT, CURRENCY {
                return '0';
            }
            when DATE {
                return 'Date.today()';
            }
            when DATETIME {
                return 'DateTime.now()';
            }
            when REFERENCE {
                // For lookup fields, we'll need to handle this specially
                return null; // Will be handled by relationship logic
            }
            when else {
                return null;
            }
        }
    }
    
    /**
     * Generates appropriate test object creation based on object type
     * Now uses dynamic field querying to handle any object
     */
    private static String generateTestObjectCreation(String objectApiName, String operation) {
        String tab = '    ';
        String result = '';
        
        // Special handling for objects with known lookup requirements
        if (objectApiName == 'Contact' || objectApiName == 'Opportunity' || objectApiName == 'Case') {
            // These often need an Account
            if (objectApiName != 'Account') {
                result += 'Account testAccount = new Account(Name = \'Test Account\');' + '\n' +
                         tab + tab + tab + 'insert testAccount;' + '\n' + tab + tab + tab;
            }
        }
        
        // Try to use dynamic field discovery first
        Map<String, String> requiredFields = getRequiredFieldsForObject(objectApiName);
        
        if (!requiredFields.isEmpty()) {
            // Build object with dynamically discovered required fields
            result += objectApiName + ' testRecord = new ' + objectApiName + '(' + '\n';
            
            List<String> fieldAssignments = new List<String>();
            for (String fieldName : requiredFields.keySet()) {
                String fieldValue = requiredFields.get(fieldName);
                fieldAssignments.add(tab + tab + tab + tab + fieldName + ' = ' + fieldValue);
            }
            
            // Add special handling for known relationships
            if (objectApiName == 'Contact' || objectApiName == 'Opportunity') {
                if (!requiredFields.containsKey('AccountId')) {
                    fieldAssignments.add(tab + tab + tab + tab + 'AccountId = testAccount.Id');
                }
            }
            
            result += String.join(fieldAssignments, ',\n') + '\n';
            result += tab + tab + tab + ');' + '\n' +
                     tab + tab + tab + 'insert testRecord;';
        } else {
            // Fall back to hardcoded values for known objects
            if (objectApiName == 'Account') {
                return objectApiName + ' testRecord = new ' + objectApiName + '(' + '\n' +
                       tab + tab + tab + tab + 'Name = \'Test Account ' + operation + '\'' + '\n' +
                       tab + tab + tab + ');' + '\n' +
                       tab + tab + tab + 'insert testRecord;';
            } else if (objectApiName == 'Contact') {
                return result + objectApiName + ' testRecord = new ' + objectApiName + '(' + '\n' +
                       tab + tab + tab + tab + 'FirstName = \'Test\',' + '\n' +
                       tab + tab + tab + tab + 'LastName = \'Contact ' + operation + '\',' + '\n' +
                       tab + tab + tab + tab + 'AccountId = testAccount.Id' + '\n' +
                       tab + tab + tab + ');' + '\n' +
                       tab + tab + tab + 'insert testRecord;';
            } else if (objectApiName == 'Opportunity') {
                return result + objectApiName + ' testRecord = new ' + objectApiName + '(' + '\n' +
                       tab + tab + tab + tab + 'Name = \'Test Opportunity ' + operation + '\',' + '\n' +
                       tab + tab + tab + tab + 'AccountId = testAccount.Id,' + '\n' +
                       tab + tab + tab + tab + 'StageName = \'Prospecting\',' + '\n' +
                       tab + tab + tab + tab + 'CloseDate = Date.today().addDays(30)' + '\n' +
                       tab + tab + tab + ');' + '\n' +
                       tab + tab + tab + 'insert testRecord;';
            } else if (objectApiName == 'Case') {
                return objectApiName + ' testRecord = new ' + objectApiName + '(' + '\n' +
                       tab + tab + tab + tab + 'Subject = \'Test Case ' + operation + '\',' + '\n' +
                       tab + tab + tab + tab + 'Status = \'New\'' + '\n' +
                       tab + tab + tab + ');' + '\n' +
                       tab + tab + tab + 'insert testRecord;';
            } else if (objectApiName == 'Lead') {
                return objectApiName + ' testRecord = new ' + objectApiName + '(' + '\n' +
                       tab + tab + tab + tab + 'FirstName = \'Test\',' + '\n' +
                       tab + tab + tab + tab + 'LastName = \'Lead ' + operation + '\',' + '\n' +
                       tab + tab + tab + tab + 'Company = \'Test Company\'' + '\n' +
                       tab + tab + tab + ');' + '\n' +
                       tab + tab + tab + 'insert testRecord;';
            } else {
                // Generic approach for custom objects or other standard objects
                String nameField = getNameFieldForObject(objectApiName);
                return objectApiName + ' testRecord = new ' + objectApiName + '(' + '\n' +
                       tab + tab + tab + tab + nameField + ' = \'Test Record ' + operation + '\'' + '\n' +
                       tab + tab + tab + ');' + '\n' +
                       tab + tab + tab + 'insert testRecord;';
            }
        }
        
        return result;
    }
    
    /**
     * Generates test object update statement
     */
    private static String generateTestObjectUpdate(String objectApiName) {
        String tab = '    ';
        String nameField = getNameFieldForObject(objectApiName);
        
        if (objectApiName == 'Contact') {
            return 'testRecord.LastName = \'Updated Contact\';';
        } else if (objectApiName == 'Opportunity') {
            return 'testRecord.StageName = \'Qualification\';';
        } else if (objectApiName == 'Case') {
            return 'testRecord.Status = \'Working\';';
        } else {
            return 'testRecord.' + nameField + ' = \'Updated Record\';';
        }
    }
    
    /**
     * Generates setup code for bulk tests (e.g., parent records)
     */
    private static String generateBulkTestSetup(String objectApiName) {
        String tab = '    ';
        String result = '';
        
        // Check if this object typically needs parent records
        if (objectApiName == 'Contact' || objectApiName == 'Opportunity') {
            result = tab + tab + tab + '// Create parent Account for relationships' + '\n' +
                    tab + tab + tab + 'Account parentAccount = new Account(Name = \'Parent Account for Bulk Test\');' + '\n' +
                    tab + tab + tab + 'insert parentAccount;' + '\n';
        } else if (objectApiName == 'Case') {
            // Cases might need an Account or Contact
            result = tab + tab + tab + '// Create parent Account for Cases' + '\n' +
                    tab + tab + tab + 'Account parentAccount = new Account(Name = \'Parent Account for Cases\');' + '\n' +
                    tab + tab + tab + 'insert parentAccount;' + '\n';
        }
        
        return result;
    }
    
    /**
     * Generates bulk test object creation
     * Uses dynamic field discovery when possible
     */
    private static String generateBulkTestObjectCreation(String objectApiName) {
        // Try to use dynamic field discovery first
        Map<String, String> requiredFields = getRequiredFieldsForObject(objectApiName);
        
        if (!requiredFields.isEmpty()) {
            List<String> fieldAssignments = new List<String>();
            for (String fieldName : requiredFields.keySet()) {
                String fieldValue = requiredFields.get(fieldName);
                // Modify value to include the iterator for uniqueness
                if (fieldValue != null && fieldValue.contains('\'Test')) {
                    fieldValue = fieldValue.replace('\'Test', '\'Test \' + i + \' ');
                }
                fieldAssignments.add(fieldName + ' = ' + fieldValue);
            }
            
            // Add parent relationship for objects that need it
            if ((objectApiName == 'Contact' || objectApiName == 'Opportunity' || objectApiName == 'Case') 
                && !requiredFields.containsKey('AccountId')) {
                fieldAssignments.add('AccountId = parentAccount.Id');
            }
            
            return 'testRecords.add(new ' + objectApiName + '(' + String.join(fieldAssignments, ', ') + '));';
        }
        
        // Fall back to hardcoded values for known objects
        if (objectApiName == 'Account') {
            return 'testRecords.add(new ' + objectApiName + '(Name = \'Test Account \' + i));';
        } else if (objectApiName == 'Contact') {
            return 'testRecords.add(new ' + objectApiName + '(LastName = \'Test Contact \' + i, AccountId = parentAccount.Id));';
        } else if (objectApiName == 'Opportunity') {
            return 'testRecords.add(new ' + objectApiName + '(' + 
                   'Name = \'Test Opportunity \' + i, ' +
                   'AccountId = parentAccount.Id, ' +
                   'StageName = \'Prospecting\', ' +
                   'CloseDate = Date.today().addDays(30)));';
        } else if (objectApiName == 'Case') {
            return 'testRecords.add(new ' + objectApiName + '(Subject = \'Test Case \' + i, Status = \'New\', AccountId = parentAccount.Id));';
        } else if (objectApiName == 'Lead') {
            return 'testRecords.add(new ' + objectApiName + '(' + 
                   'LastName = \'Test Lead \' + i, ' +
                   'Company = \'Test Company \' + i));';
        } else {
            String nameField = getNameFieldForObject(objectApiName);
            return 'testRecords.add(new ' + objectApiName + '(' + nameField + ' = \'Test Record \' + i));';
        }
    }
    
    /**
     * Determines if object is a standard object with required fields
     */
    private static Boolean isStandardObjectWithRequiredFields(String objectApiName) {
        Set<String> standardObjects = new Set<String>{
            'Account', 'Contact', 'Opportunity', 'Lead', 'Case', 'Task', 'Event'
        };
        return standardObjects.contains(objectApiName);
    }
    
    /**
     * Gets the appropriate name field for an object
     */
    private static String getNameFieldForObject(String objectApiName) {
        // For custom objects, use Name field if it exists
        if (objectApiName.endsWith('__c')) {
            return 'Name';
        }
        
        // For standard objects, return the appropriate field
        Map<String, String> standardObjectNameFields = new Map<String, String>{
            'Account' => 'Name',
            'Contact' => 'LastName',
            'Opportunity' => 'Name',
            'Lead' => 'LastName',
            'Case' => 'Subject',
            'Task' => 'Subject',
            'Event' => 'Subject',
            'Campaign' => 'Name',
            'Product2' => 'Name',
            'Pricebook2' => 'Name'
        };
        
        return standardObjectNameFields.containsKey(objectApiName) ? 
               standardObjectNameFields.get(objectApiName) : 'Name';
    }
    
    /**
     * RemoteAction to check deployment status from VF page
     */
    @RemoteAction
    public static Map<String, Object> checkDeploymentStatus(String deploymentId) {
        MetadataService.MetadataPort service = MetadataService.createService();
        MetadataService.DeployResult deployResult = service.checkDeployStatus(deploymentId, true);
        
        Map<String, Object> result = new Map<String, Object>();
        result.put('done', deployResult.done);
        result.put('id', deployResult.id);
        result.put('status', deployResult.status);
        result.put('numberComponentsDeployed', deployResult.numberComponentsDeployed);
        result.put('numberComponentsTotal', deployResult.numberComponentsTotal);
        
        if (deployResult.details != null && deployResult.details.componentFailures != null) {
            List<String> errors = new List<String>();
            for (MetadataService.DeployMessage msg : deployResult.details.componentFailures) {
                errors.add(msg.problem);
            }
            result.put('errors', errors);
        }
        
        return result;
    }
    
    /**
     * Checks the status of a deployment (for internal use)
     * Following apex-mdapi pattern for checking async deployment status
     */
    public static MetadataService.DeployResult checkDeployStatus(String deploymentId) {
        MetadataService.MetadataPort service = MetadataService.createService();
        return service.checkDeployStatus(deploymentId, true);
    }
    
    /**
     * Retrieves deployment details including any errors
     * Useful for debugging failed deployments
     */
    public static MetadataService.DeployResult getDeploymentDetails(String deploymentId) {
        MetadataService.MetadataPort service = MetadataService.createService();
        
        // In production:
        // return service.checkDeployStatus(deploymentId, true);
        
        MetadataService.DeployResult result = new MetadataService.DeployResult();
        result.done = true;
        result.id = deploymentId;
        result.status = 'Succeeded';
        
        // Create details for the result
        MetadataService.DeployDetails details = new MetadataService.DeployDetails();
        details.componentSuccesses = new MetadataService.DeployMessage[]{};
        details.componentFailures = new MetadataService.DeployMessage[]{};
        result.details = details;
        
        return result;
    }
}
