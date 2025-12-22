public class FlexiPageToolingService {
    // Defines the version of the Tooling API to use (kept for reference)
    public static final String TOOLING_API_VERSION = '60.0';

    /**
     * Retrieves the metadata for a specified FlexiPage based on its developer name.
     *
     * @param developerName The developer name of the FlexiPage to retrieve metadata for.
     * @return A JSON string representing the metadata of the specified FlexiPage.
     * @throws AuraHandledException if an error occurs while retrieving the metadata.
     */
    @AuraEnabled
    public static String getFlexiPageMetadata(String developerName) {
        try {
            // Always use the Metadata API service - no more Tooling API
            return FlexiPageMetadataService.getFlexiPageMetadata(developerName);
        } catch (Exception e) {
            throw new AuraHandledException('Exception in getFlexiPageMetadata: ' + e.getMessage());
        }
    }
    
    /**
     * Retrieves the metadata for a specified FlexiPage using the Tooling API.
     * This is the original implementation kept for reference - NOT USED.
     *
     * @param developerName The developer name of the FlexiPage to retrieve metadata for.
     * @return A JSON string representing the metadata of the specified FlexiPage.
     * @throws AuraHandledException if an error occurs while retrieving the metadata.
     */
    private static String getFlexiPageMetadataViaToolingAPI(String developerName) {
        try {
            // Log entry into the method
            // Logger.info('Entering getFlexiPageMetadata method');

            // Construct the SOQL query to fetch metadata for the specified FlexiPage
            String query = 'SELECT Metadata FROM FlexiPage WHERE DeveloperName = \'' + developerName + '\'';
            // Logger.info('Query: ' + query);

            // Setup the HTTP request for the Tooling API call
            HttpRequest request = new HttpRequest();
            request.setEndpoint('callout:Tooling_API_Credential/services/data/v' + TOOLING_API_VERSION + '/tooling/query/?q=' + EncodingUtil.urlEncode(query, 'UTF-8'));
            request.setMethod('GET');
            request.setHeader('Accept', 'application/json');
            request.setHeader('Content-Type', 'application/json');
            // Logger.info('HttpRequest: ' + request);

            // Send the HTTP request
            Http http = new Http();
            HttpResponse response = http.send(request);
            // Logger.info('HttpResponse Status: ' + response.getStatus());
            // Logger.info('HttpResponse Body: ' + response.getBody());

            // Handle redirect responses (301/302)
            if (response.getStatusCode() == 301 || response.getStatusCode() == 302) {
                String location = response.getHeader('Location');
                if (String.isNotBlank(location)) {
                    // Create new request with redirect location
                    HttpRequest redirectRequest = new HttpRequest();
                    redirectRequest.setEndpoint(location);
                    redirectRequest.setMethod('GET');
                    // Add authorization header if not already in URL
                    if (!location.contains('oauth_token=')) {
                        redirectRequest.setHeader('Authorization', request.getHeader('Authorization'));
                    }
                    
                    // Send redirect request
                    HttpResponse redirectResponse = http.send(redirectRequest);
                    
                    if (redirectResponse.getStatusCode() == 200) {
                        return handleFlexiPageResponse(redirectResponse.getBody());
                    } else {
                        throw new AuraHandledException('Error after redirect: ' + redirectResponse.getStatus() + ' - ' + redirectResponse.getBody());
                    }
                } else {
                    throw new AuraHandledException('Redirect response missing Location header');
                }
            } else if (response.getStatusCode() == 200) {
                // Handle the successful response
                return handleFlexiPageResponse(response.getBody());
            } else {
                // Log and throw an error for unsuccessful responses
                // Logger.error('Error response from API: ' + response.getStatus());
                // Logger.error('Error response body: ' + response.getBody());
                throw new AuraHandledException('Error retrieving FlexiPage metadata: ' + response.getStatus() + ' - ' + response.getBody());
            }
        } catch (Exception e) {
            // Log and throw an exception if an error occurs
            // Logger.error('Exception in getFlexiPageMetadata', e);
            throw new AuraHandledException('Exception in getFlexiPageMetadataViaToolingAPI: ' + e.getMessage());
        } finally {
            // Ensure logs are saved
            // Logger.saveLog();
        }
    }

    /**
     * Handles the response from the Tooling API and extracts the FlexiPage metadata.
     *
     * @param responseBody The response body from the Tooling API call.
     * @return A JSON string representing the metadata of the FlexiPage.
     * @throws AuraHandledException if an error occurs while processing the response.
     */
    private static String handleFlexiPageResponse(String responseBody) {
        try {
            // Log entry into the method
            // Logger.info('Entering handleFlexiPageResponse method');
            // Logger.info('Response Body: ' + responseBody);

            // Parse the JSON response body into a Map
            Map<String, Object> result = (Map<String, Object>) JSON.deserializeUntyped(responseBody);
            // Logger.info('Response body parsed: ' + JSON.serialize(result));

            // Extract records from the result
            List<Object> records = (List<Object>) result.get('records');
            if (records != null && !records.isEmpty()) {
                // Extract metadata from the first record
                Map<String, Object> firstRecord = (Map<String, Object>) records[0];
                Map<String, Object> metadata = (Map<String, Object>) firstRecord.get('Metadata');
                // Logger.info('Metadata: ' + JSON.serialize(metadata));

                // Return the metadata as a JSON string
                return JSON.serialize(metadata);
            } else {
                // Throw an error if no records are found
                throw new AuraHandledException('No records found for the specified FlexiPage');
            }
        } catch (Exception e) {
            // Log and throw an exception if an error occurs
            // Logger.error('Exception in handleFlexiPageResponse', e);
            throw new AuraHandledException('Exception in handleFlexiPageResponse: ' + e.getMessage());
        } finally {
            // Ensure logs are saved
            // Logger.saveLog();
        }
    }

    /**
     * Retrieves the field values for a specified record in a specified object.
     *
     * @param recordId The ID of the record to retrieve field values for.
     * @param objectApiName The API name of the object the record belongs to.
     * @return A Map of field API names to their corresponding values for the specified record.
     * @throws AuraHandledException if an error occurs while retrieving the field values.
     */
    @AuraEnabled(cacheable=true)
    public static Map<String, Object> getFieldValues(String recordId, String objectApiName, List<String> fieldApiNames) {
        Map<String, Object> result = new Map<String, Object>();
        Map<String, Object> fieldValues = new Map<String, Object>();
        Map<String, Object> fieldMetadata = new Map<String, Object>();
        
        try {
            // Retrieve the schema for the specified object
            Schema.SObjectType sObjectType = Schema.getGlobalDescribe().get(objectApiName);
            if (sObjectType == null) {
                throw new IllegalArgumentException('Invalid object API name: ' + objectApiName);
            }

            // Get the field map for the object
            Map<String, Schema.SObjectField> fieldMap = sObjectType.getDescribe().fields.getMap();
            
            // Build list of fields to query (use provided list or all fields)
            Set<String> fieldsToQuery = new Set<String>();
            if (fieldApiNames != null && !fieldApiNames.isEmpty()) {
                // Normalize field names to lowercase for lookup
                for (String fieldName : fieldApiNames) {
                    String normalizedName = fieldName.toLowerCase();
                    if (fieldMap.containsKey(normalizedName)) {
                        fieldsToQuery.add(normalizedName);
                    }
                }
                
                // Always include system fields to ensure they're available
                Set<String> systemFields = new Set<String>{'createdbyid', 'createddate', 'lastmodifiedbyid', 'lastmodifieddate', 'systemmodstamp', 'ownerid'};
                for (String sysField : systemFields) {
                    if (fieldMap.containsKey(sysField) && !fieldsToQuery.contains(sysField)) {
                        fieldsToQuery.add(sysField);
                    }
                }
            } else {
                fieldsToQuery.addAll(fieldMap.keySet());
            }

            // Construct the SOQL query
            String query = 'SELECT ' + String.join(new List<String>(fieldsToQuery), ',') + ' FROM ' + objectApiName + ' WHERE Id = :recordId';

            // Execute the query
            SObject record = Database.query(query);

            // Populate field values and metadata
            for (String fieldApiName : fieldsToQuery) {
                Schema.DescribeFieldResult fieldDescribe = fieldMap.get(fieldApiName).getDescribe();
                
                // Get field value
                Object fieldValue = record.get(fieldApiName);
                fieldValues.put(fieldApiName, fieldValue);
                
                // Build field metadata
                Map<String, Object> fieldInfo = new Map<String, Object>();
                fieldInfo.put('label', fieldDescribe.getLabel());
                fieldInfo.put('type', String.valueOf(fieldDescribe.getType()));
                fieldInfo.put('isNameField', fieldDescribe.isNameField());
                
                // Debug logging for Owner field
                if (fieldApiName.equalsIgnoreCase('ownerid')) {
                    System.debug('Processing OwnerId field:');
                    System.debug('  - Field value: ' + fieldValue);
                    System.debug('  - Field type: ' + fieldDescribe.getType());
                }
                
                // Add reference info for lookup fields
                if (fieldDescribe.getType() == Schema.DisplayType.REFERENCE) {
                    List<Schema.SObjectType> referenceTypes = fieldDescribe.getReferenceTo();
                    if (!referenceTypes.isEmpty()) {
                        fieldInfo.put('referenceObjectName', referenceTypes[0].getDescribe().getName());
                        
                        // If we have a value, get the name field value for display
                        if (fieldValue != null) {
                            try {
                                String refObjectName = referenceTypes[0].getDescribe().getName();
                                String nameField = getNameField(refObjectName);
                                
                                // Special handling for User reference fields
                                if (fieldApiName.equalsIgnoreCase('ownerid') || 
                                    fieldApiName.equalsIgnoreCase('createdbyid') || 
                                    fieldApiName.equalsIgnoreCase('lastmodifiedbyid')) {
                                    
                                    String userId = String.valueOf(fieldValue);
                                    if (fieldApiName.equalsIgnoreCase('ownerid')) {
                                        // Owner can be User or Group
                                        if (userId.startsWith('005')) {
                                            // It's a User
                                            String userQuery = 'SELECT Name FROM User WHERE Id = :fieldValue LIMIT 1';
                                            List<User> users = Database.query(userQuery);
                                            if (!users.isEmpty()) {
                                                fieldInfo.put('referenceNameValue', users[0].Name);
                                                System.debug('OwnerId User found: ' + users[0].Name);
                                            } else {
                                                System.debug('OwnerId User NOT found for ID: ' + fieldValue);
                                            }
                                        } else if (userId.startsWith('00G')) {
                                            // It's a Group (Queue)
                                            String groupQuery = 'SELECT Name FROM Group WHERE Id = :fieldValue LIMIT 1';
                                            List<Group> groups = Database.query(groupQuery);
                                            if (!groups.isEmpty()) {
                                                fieldInfo.put('referenceNameValue', groups[0].Name);
                                            }
                                        }
                                    } else {
                                        // CreatedById and LastModifiedById are always Users
                                        String userQuery = 'SELECT Name FROM User WHERE Id = :fieldValue LIMIT 1';
                                        List<User> users = Database.query(userQuery);
                                        if (!users.isEmpty()) {
                                            fieldInfo.put('referenceNameValue', users[0].Name);
                                            System.debug('OwnerId User found: ' + users[0].Name);
                                        } else {
                                            System.debug('OwnerId User NOT found for ID: ' + fieldValue);
                                        }
                                    }
                                } else {
                                    // Standard lookup handling
                                    String refQuery = 'SELECT ' + nameField + ' FROM ' + refObjectName + ' WHERE Id = :fieldValue LIMIT 1';
                                    List<SObject> refRecords = Database.query(refQuery);
                                    if (!refRecords.isEmpty()) {
                                        fieldInfo.put('referenceNameValue', refRecords[0].get(nameField));
                                    }
                                }
                            } catch (Exception e) {
                                // If we can't get the reference name, just log and continue
                                System.debug('Error getting reference name for field ' + fieldApiName + ': ' + e.getMessage());
                            }
                        }
                    }
                }
                
                // Store metadata with lowercase key for consistent lookup
                fieldMetadata.put(fieldApiName.toLowerCase(), fieldInfo);
            }
            
            result.put('values', fieldValues);
            result.put('metadata', fieldMetadata);

        } catch (Exception e) {
            throw new AuraHandledException('Error retrieving field values: ' + e.getMessage());
        }
        
        return result;
    }
    
    /**
     * Get the name field for an object
     */
    private static String getNameField(String objectApiName) {
        Schema.SObjectType sObjectType = Schema.getGlobalDescribe().get(objectApiName);
        if (sObjectType != null) {
            Map<String, Schema.SObjectField> fields = sObjectType.getDescribe().fields.getMap();
            for (String fieldName : fields.keySet()) {
                if (fields.get(fieldName).getDescribe().isNameField()) {
                    return fieldName;
                }
            }
        }
        return 'Name'; // Default fallback
    }
}