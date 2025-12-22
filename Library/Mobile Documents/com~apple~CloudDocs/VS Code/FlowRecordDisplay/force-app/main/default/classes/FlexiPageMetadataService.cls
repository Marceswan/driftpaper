/**
 * @description Service class for retrieving FlexiPage metadata using Metadata API
 * @author Marc Swan
 * @date 2025-01-19
 */
public with sharing class FlexiPageMetadataService {
    
    /**
     * @description Inner class to represent field information from FlexiPage
     */
    public class FieldInfo {
        @AuraEnabled public String section;
        @AuraEnabled public Integer position;
        @AuraEnabled public String fieldName;
        @AuraEnabled public Map<String, Object> visibilityRules;
        
        public FieldInfo(String section, Integer position, String fieldName, Map<String, Object> visibilityRules) {
            this.section = section;
            this.position = position;
            this.fieldName = fieldName;
            this.visibilityRules = visibilityRules;
        }
    }
    
    /**
     * @description Retrieves FlexiPage metadata for a given developer name and returns field information
     * @param developerName The API name of the FlexiPage to retrieve
     * @return List of FieldInfo objects containing field details from the FlexiPage
     * @example FlexiPageMetadataService.getFlexiPageFields('Account_Record_Page')
     */
    @AuraEnabled(cacheable=true)
    public static List<FieldInfo> getFlexiPageFields(String developerName) {
        List<FieldInfo> fieldInfoList = new List<FieldInfo>();
        
        try {
            // Validate input
            if (String.isBlank(developerName)) {
                throw new FlexiPageMetadataException('Developer name cannot be blank');
            }
            
            // Retrieve metadata using internal API approach
            String metadataJson = getFlexiPageMetadata(developerName);
            Map<String, Object> responseMap = (Map<String, Object>) JSON.deserializeUntyped(metadataJson);
            
            if (responseMap.get('success') == true && responseMap.containsKey('metadata')) {
                Map<String, Object> metadata = (Map<String, Object>) responseMap.get('metadata');
                fieldInfoList = processFlexiPageMetadata(metadata);
            }
            
            return fieldInfoList;
            
        } catch (Exception e) {
            System.debug('Exception in getFlexiPageFields: ' + e.getMessage());
            System.debug('Stack trace: ' + e.getStackTraceString());
            throw new AuraHandledException('Error retrieving FlexiPage metadata: ' + e.getMessage());
        }
    }
    
    /**
     * @description Retrieves FlexiPage metadata for a given developer name (JSON format)
     * This method uses internal APIs to retrieve FlexiPage configuration
     * @param developerName The API name of the FlexiPage to retrieve
     * @return JSON string containing the FlexiPage metadata
     * @example FlexiPageMetadataService.getFlexiPageMetadata('Account_Record_Page')
     */
    @AuraEnabled(cacheable=true)
    public static String getFlexiPageMetadata(String developerName) {
        try {
            // Validate input
            if (String.isBlank(developerName)) {
                throw new FlexiPageMetadataException('Developer name cannot be blank');
            }
            
            // Since direct Metadata API access for FlexiPage is not available,
            // we'll use an alternative approach through REST API
            Map<String, Object> flexiPageData = retrieveFlexiPageData(developerName);
            
            if (flexiPageData != null) {
                // Return the metadata directly as the LWC expects
                return JSON.serialize(flexiPageData);
            } else {
                throw new AuraHandledException('No FlexiPage found with developer name: ' + developerName);
            }
            
        } catch (Exception e) {
            throw new AuraHandledException('Exception in getFlexiPageMetadata: ' + e.getMessage());
        }
    }
    
    /**
     * @description Retrieves FlexiPage data using Tooling API REST approach
     * @param developerName The developer name of the FlexiPage
     * @return Map containing the FlexiPage metadata
     */
    private static Map<String, Object> retrieveFlexiPageData(String developerName) {
        try {
            // Construct SOQL query for Tooling API
            String query = 'SELECT Id, Metadata FROM FlexiPage WHERE DeveloperName = \'' + developerName + '\'';
            String encodedQuery = EncodingUtil.urlEncode(query, 'UTF-8');
            
            // Use Tooling API REST endpoint
            HttpRequest request = new HttpRequest();
            String endpoint = URL.getOrgDomainUrl().toExternalForm() + 
                             '/services/data/v60.0/tooling/query/?q=' + encodedQuery;
            
            request.setEndpoint(endpoint);
            request.setMethod('GET');
            request.setHeader('Authorization', 'Bearer ' + UserInfo.getSessionId());
            request.setHeader('Content-Type', 'application/json');
            request.setHeader('Accept', 'application/json');
            
            Http http = new Http();
            HttpResponse response = http.send(request);
            
            System.debug('Response status: ' + response.getStatusCode());
            System.debug('Response body: ' + response.getBody());
            
            if (response.getStatusCode() == 200) {
                // Parse the response
                Map<String, Object> responseMap = (Map<String, Object>) JSON.deserializeUntyped(response.getBody());
                List<Object> records = (List<Object>) responseMap.get('records');
                
                if (records != null && !records.isEmpty()) {
                    Map<String, Object> firstRecord = (Map<String, Object>) records[0];
                    Map<String, Object> metadata = (Map<String, Object>) firstRecord.get('Metadata');
                    
                    // Return in expected format
                    Map<String, Object> resultMap = new Map<String, Object>();
                    resultMap.put('success', true);
                    resultMap.put('metadata', metadata);
                    
                    return resultMap;
                } else {
                    throw new AuraHandledException('No FlexiPage found with developer name: ' + developerName);
                }
            } else {
                throw new AuraHandledException('Error retrieving FlexiPage metadata: ' + response.getStatus() + ' - ' + response.getBody());
            }
            
        } catch (Exception e) {
            System.debug('Error retrieving FlexiPage: ' + e.getMessage());
            System.debug('Stack trace: ' + e.getStackTraceString());
            throw new AuraHandledException('Error retrieving FlexiPage: ' + e.getMessage());
        }
    }
    
    
    /**
     * @description Processes FlexiPage metadata to extract field information
     * @param metadata Map containing FlexiPage metadata
     * @return List of FieldInfo objects
     */
    private static List<FieldInfo> processFlexiPageMetadata(Map<String, Object> metadata) {
        List<FieldInfo> fieldInfoList = new List<FieldInfo>();
        
        if (metadata.containsKey('flexiPageRegions')) {
            List<Object> regions = (List<Object>) metadata.get('flexiPageRegions');
            
            for (Object regionObj : regions) {
                Map<String, Object> region = (Map<String, Object>) regionObj;
                String sectionName = (String) region.get('name');
                
                if (region.containsKey('itemInstances')) {
                    List<Object> items = (List<Object>) region.get('itemInstances');
                    Integer position = 1;
                    
                    for (Object itemObj : items) {
                        Map<String, Object> item = (Map<String, Object>) itemObj;
                        
                        if (item.containsKey('fieldInstance')) {
                            Map<String, Object> fieldInstance = (Map<String, Object>) item.get('fieldInstance');
                            String fieldName = (String) fieldInstance.get('fieldItem');
                            Map<String, Object> visibilityRules = null;
                            
                            // Parse visibility rules if present
                            if (fieldInstance.containsKey('visibilityRule')) {
                                visibilityRules = parseVisibilityRule((Map<String, Object>) fieldInstance.get('visibilityRule'));
                            }
                            
                            fieldInfoList.add(new FieldInfo(sectionName, position, fieldName, visibilityRules));
                        }
                        position++;
                    }
                }
            }
        }
        
        return fieldInfoList;
    }
    
    /**
     * @description Parses visibility rule metadata into a map structure
     * @param visibilityRule The visibility rule map
     * @return Map containing visibility rule information
     */
    private static Map<String, Object> parseVisibilityRule(Map<String, Object> visibilityRule) {
        Map<String, Object> visibilityMap = new Map<String, Object>();
        
        if (visibilityRule.containsKey('booleanFilter')) {
            visibilityMap.put('booleanFilter', visibilityRule.get('booleanFilter'));
        }
        
        if (visibilityRule.containsKey('criteria')) {
            List<Object> criteriaObj = (List<Object>) visibilityRule.get('criteria');
            List<Map<String, String>> criteriaList = new List<Map<String, String>>();
            
            for (Object criterion : criteriaObj) {
                Map<String, Object> criterionMap = (Map<String, Object>) criterion;
                Map<String, String> criterionResult = new Map<String, String>();
                criterionResult.put('leftValue', String.valueOf(criterionMap.get('field')));
                criterionResult.put('operator', String.valueOf(criterionMap.get('operation')));
                criterionResult.put('rightValue', String.valueOf(criterionMap.get('value')));
                criteriaList.add(criterionResult);
            }
            
            visibilityMap.put('criteria', criteriaList);
        }
        
        return visibilityMap;
    }
    
    /**
     * @description Custom exception class for FlexiPage metadata operations
     */
    public class FlexiPageMetadataException extends Exception {}
}