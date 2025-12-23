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
