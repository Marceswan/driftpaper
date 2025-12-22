/**
 * @description Controller for Account Hierarchy Tree LWC.
 *              Returns nested tree structure for lightning-tree-grid display.
 */
public with sharing class AccountHierarchyController {

    /**
     * @description Wrapper class for hierarchy tree nodes
     */
    public class HierarchyNode {
        @AuraEnabled public String id;
        @AuraEnabled public Map<String, Object> fields;
        @AuraEnabled public List<HierarchyNode> children;

        public HierarchyNode() {
            this.fields = new Map<String, Object>();
            this.children = new List<HierarchyNode>();
        }
    }

    /**
     * @description Result wrapper containing tree and current account marker
     */
    public class HierarchyResult {
        @AuraEnabled public List<HierarchyNode> nodes;
        @AuraEnabled public String currentAccountId;

        public HierarchyResult() {
            this.nodes = new List<HierarchyNode>();
        }
    }

    /**
     * @description Get the full account hierarchy for display in tree-grid
     * @param recordId The current Account record Id
     * @param fields List of field API names to include in the result
     * @return HierarchyResult containing nested tree structure
     */
    @AuraEnabled(cacheable=true)
    public static HierarchyResult getAccountHierarchy(Id recordId, List<String> fields) {
        HierarchyResult result = new HierarchyResult();
        result.currentAccountId = recordId;

        if (recordId == null) {
            return result;
        }

        // Validate fields list - ensure we always have Id and Name
        Set<String> fieldSet = new Set<String>{'Id', 'Name', 'ParentId'};
        if (fields != null) {
            for (String field : fields) {
                if (String.isNotBlank(field)) {
                    fieldSet.add(field.trim());
                }
            }
        }

        // Build account map using existing service
        Map<Id, Account> accountMap = AccountHierarchyService.buildAccountMap(new Set<Id>{recordId});

        if (accountMap.isEmpty()) {
            return result;
        }

        // Find ultimate parent
        Id rootId = AccountHierarchyService.findUltimateParent(recordId, accountMap);

        if (rootId == null) {
            return result;
        }

        // Query accounts with requested fields
        List<Account> accounts = queryAccountsWithFields(accountMap.keySet(), fieldSet);
        Map<Id, Account> accountDataMap = new Map<Id, Account>(accounts);

        // Build parent-to-children map
        Map<Id, List<Id>> parentToChildrenMap = buildParentToChildrenMap(accountDataMap);

        // Build tree starting from root
        HierarchyNode rootNode = buildTreeNode(rootId, accountDataMap, parentToChildrenMap, fieldSet);
        if (rootNode != null) {
            result.nodes.add(rootNode);
        }

        return result;
    }

    /**
     * @description Query accounts with specified fields
     */
    private static List<Account> queryAccountsWithFields(Set<Id> accountIds, Set<String> fields) {
        // Build dynamic SOQL
        List<String> fieldList = new List<String>(fields);
        String soql = 'SELECT ' + String.join(fieldList, ', ') + ' FROM Account WHERE Id IN :accountIds WITH SECURITY_ENFORCED';

        return Database.query(soql);
    }

    /**
     * @description Build map of parent Id to list of child Ids
     */
    private static Map<Id, List<Id>> buildParentToChildrenMap(Map<Id, Account> accountMap) {
        Map<Id, List<Id>> parentToChildrenMap = new Map<Id, List<Id>>();

        for (Account acc : accountMap.values()) {
            if (acc.ParentId != null) {
                if (!parentToChildrenMap.containsKey(acc.ParentId)) {
                    parentToChildrenMap.put(acc.ParentId, new List<Id>());
                }
                parentToChildrenMap.get(acc.ParentId).add(acc.Id);
            }
        }

        return parentToChildrenMap;
    }

    /**
     * @description Recursively build tree node structure
     */
    private static HierarchyNode buildTreeNode(
        Id accountId,
        Map<Id, Account> accountDataMap,
        Map<Id, List<Id>> parentToChildrenMap,
        Set<String> fields
    ) {
        Account acc = accountDataMap.get(accountId);
        if (acc == null) {
            return null;
        }

        HierarchyNode node = new HierarchyNode();
        node.id = acc.Id;

        // Populate fields
        for (String field : fields) {
            if (field != 'ParentId') {
                try {
                    Object value = acc.get(field);
                    node.fields.put(field, value);
                } catch (Exception e) {
                    // Field may not be accessible or doesn't exist
                    node.fields.put(field, null);
                }
            }
        }

        // Recursively add children
        List<Id> childIds = parentToChildrenMap.get(accountId);
        if (childIds != null) {
            for (Id childId : childIds) {
                HierarchyNode childNode = buildTreeNode(childId, accountDataMap, parentToChildrenMap, fields);
                if (childNode != null) {
                    node.children.add(childNode);
                }
            }
        }

        return node;
    }
}
