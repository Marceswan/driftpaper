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

        // Build query with LIMIT to prevent fetching all opportunities
        Integer queryLimit = validLimit * 2 + 10;
        String fieldList = String.join(new List<String>(fieldSet), ', ');
        String query = 'SELECT ' + fieldList + ', ' +
            '(SELECT Id, Product2.Name, Quantity, UnitPrice, TotalPrice, Description ' +
            'FROM OpportunityLineItems ORDER BY Product2.Name ASC) ' +
            'FROM Opportunity ' +
            'WHERE AccountId = :accountId ' +
            'WITH SECURITY_ENFORCED ' +
            'ORDER BY ' + validSortField + ' ' + validSortDirection + ' NULLS LAST ' +
            'LIMIT ' + queryLimit;

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
                } catch (System.SObjectException e) {
                    // Field not accessible due to FLS or doesn't exist
                    System.debug(LoggingLevel.FINE, 'Field not accessible: ' + field + ' - ' + e.getMessage());
                } catch (Exception e) {
                    // Unexpected error accessing field
                    System.debug(LoggingLevel.WARN, 'Error accessing field: ' + field + ' - ' + e.getMessage());
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
}
