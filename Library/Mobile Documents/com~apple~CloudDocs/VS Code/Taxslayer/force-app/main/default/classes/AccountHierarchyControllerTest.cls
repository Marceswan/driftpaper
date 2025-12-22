/**
 * @description Test class for AccountHierarchyController
 *              Targets 90%+ code coverage with comprehensive hierarchy scenarios
 */
@IsTest
private class AccountHierarchyControllerTest {

    @TestSetup
    static void setupTestData() {
        // Create a 4-level hierarchy: Root -> Level1 -> Level2 -> Level3
        Account root = new Account(Name = 'Test Root', Industry = 'Technology');
        insert root;

        Account level1 = new Account(Name = 'Test Level 1', ParentId = root.Id, Industry = 'Finance');
        insert level1;

        Account level2 = new Account(Name = 'Test Level 2', ParentId = level1.Id, Industry = 'Healthcare');
        insert level2;

        Account level3 = new Account(Name = 'Test Level 3', ParentId = level2.Id, Industry = 'Retail');
        insert level3;

        // Create a sibling at level 1
        Account sibling = new Account(Name = 'Test Sibling', ParentId = root.Id, Industry = 'Manufacturing');
        insert sibling;
    }

    @IsTest
    static void testGetAccountHierarchy_FromRoot() {
        Account root = [SELECT Id FROM Account WHERE Name = 'Test Root' LIMIT 1];

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(root.Id, new List<String>{'Name', 'Industry'});
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.assertEquals(root.Id, result.currentAccountId, 'Current account should be root');
        System.assertEquals(1, result.nodes.size(), 'Should have 1 root node');

        AccountHierarchyController.HierarchyNode rootNode = result.nodes[0];
        System.assertEquals(root.Id, rootNode.id, 'Root node Id should match');
        System.assertEquals('Test Root', rootNode.fields.get('Name'), 'Name should be populated');
        System.assertEquals('Technology', rootNode.fields.get('Industry'), 'Industry should be populated');
        System.assertEquals(2, rootNode.children.size(), 'Root should have 2 children (Level1 and Sibling)');
    }

    @IsTest
    static void testGetAccountHierarchy_FromMiddle() {
        Account level2 = [SELECT Id FROM Account WHERE Name = 'Test Level 2' LIMIT 1];
        Account root = [SELECT Id FROM Account WHERE Name = 'Test Root' LIMIT 1];

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(level2.Id, new List<String>{'Name'});
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.assertEquals(level2.Id, result.currentAccountId, 'Current account should be level2');
        System.assertEquals(1, result.nodes.size(), 'Should have 1 root node');
        System.assertEquals(root.Id, result.nodes[0].id, 'Tree should start from ultimate parent');
    }

    @IsTest
    static void testGetAccountHierarchy_FromLeaf() {
        Account level3 = [SELECT Id FROM Account WHERE Name = 'Test Level 3' LIMIT 1];
        Account root = [SELECT Id FROM Account WHERE Name = 'Test Root' LIMIT 1];

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(level3.Id, new List<String>{'Name'});
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.assertEquals(level3.Id, result.currentAccountId, 'Current account should be level3');
        System.assertEquals(1, result.nodes.size(), 'Should have 1 root node');
        System.assertEquals(root.Id, result.nodes[0].id, 'Tree should start from ultimate parent');

        // Verify full depth exists
        AccountHierarchyController.HierarchyNode rootNode = result.nodes[0];
        System.assert(rootNode.children.size() > 0, 'Root should have children');
    }

    @IsTest
    static void testGetAccountHierarchy_StandaloneAccount() {
        Account standalone = new Account(Name = 'Standalone Account');
        insert standalone;

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(standalone.Id, new List<String>{'Name'});
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.assertEquals(standalone.Id, result.currentAccountId, 'Current account should be standalone');
        System.assertEquals(1, result.nodes.size(), 'Should have 1 node');
        System.assertEquals(standalone.Id, result.nodes[0].id, 'Node should be the standalone account');
        System.assertEquals(0, result.nodes[0].children.size(), 'Standalone should have no children');
    }

    @IsTest
    static void testGetAccountHierarchy_NullRecordId() {
        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(null, new List<String>{'Name'});
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.assertEquals(null, result.currentAccountId, 'Current account should be null');
        System.assertEquals(0, result.nodes.size(), 'Should have no nodes');
    }

    @IsTest
    static void testGetAccountHierarchy_NullFields() {
        Account root = [SELECT Id FROM Account WHERE Name = 'Test Root' LIMIT 1];

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(root.Id, null);
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.assertEquals(1, result.nodes.size(), 'Should have 1 root node');
        // Should still have Id and Name (defaults)
        System.assertNotEquals(null, result.nodes[0].fields.get('Name'), 'Name should be populated');
    }

    @IsTest
    static void testGetAccountHierarchy_EmptyFields() {
        Account root = [SELECT Id FROM Account WHERE Name = 'Test Root' LIMIT 1];

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(root.Id, new List<String>());
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.assertEquals(1, result.nodes.size(), 'Should have 1 root node');
    }

    @IsTest
    static void testGetAccountHierarchy_WithBlankFields() {
        Account root = [SELECT Id FROM Account WHERE Name = 'Test Root' LIMIT 1];

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(root.Id, new List<String>{'Name', '', '  ', 'Industry'});
        Test.stopTest();

        System.assertNotEquals(null, result, 'Result should not be null');
        System.assertEquals(1, result.nodes.size(), 'Should have 1 root node');
        System.assertEquals('Test Root', result.nodes[0].fields.get('Name'), 'Name should be populated');
        System.assertEquals('Technology', result.nodes[0].fields.get('Industry'), 'Industry should be populated');
    }

    @IsTest
    static void testGetAccountHierarchy_TreeStructure() {
        Account root = [SELECT Id FROM Account WHERE Name = 'Test Root' LIMIT 1];
        Account level1 = [SELECT Id FROM Account WHERE Name = 'Test Level 1' LIMIT 1];
        Account level2 = [SELECT Id FROM Account WHERE Name = 'Test Level 2' LIMIT 1];
        Account level3 = [SELECT Id FROM Account WHERE Name = 'Test Level 3' LIMIT 1];
        Account sibling = [SELECT Id FROM Account WHERE Name = 'Test Sibling' LIMIT 1];

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(level3.Id, new List<String>{'Name'});
        Test.stopTest();

        // Navigate tree to verify structure
        AccountHierarchyController.HierarchyNode rootNode = result.nodes[0];
        System.assertEquals(root.Id, rootNode.id, 'Root should be at top');

        // Find Level1 in root's children
        AccountHierarchyController.HierarchyNode level1Node = null;
        AccountHierarchyController.HierarchyNode siblingNode = null;
        for (AccountHierarchyController.HierarchyNode child : rootNode.children) {
            if (child.id == level1.Id) {
                level1Node = child;
            } else if (child.id == sibling.Id) {
                siblingNode = child;
            }
        }

        System.assertNotEquals(null, level1Node, 'Level1 should be child of root');
        System.assertNotEquals(null, siblingNode, 'Sibling should be child of root');
        System.assertEquals(0, siblingNode.children.size(), 'Sibling should have no children');

        // Verify Level1 -> Level2 -> Level3
        System.assertEquals(1, level1Node.children.size(), 'Level1 should have 1 child');
        AccountHierarchyController.HierarchyNode level2Node = level1Node.children[0];
        System.assertEquals(level2.Id, level2Node.id, 'Level2 should be child of Level1');

        System.assertEquals(1, level2Node.children.size(), 'Level2 should have 1 child');
        AccountHierarchyController.HierarchyNode level3Node = level2Node.children[0];
        System.assertEquals(level3.Id, level3Node.id, 'Level3 should be child of Level2');
        System.assertEquals(0, level3Node.children.size(), 'Level3 should have no children');
    }

    @IsTest
    static void testHierarchyNodeConstructor() {
        Test.startTest();
        AccountHierarchyController.HierarchyNode node = new AccountHierarchyController.HierarchyNode();
        Test.stopTest();

        System.assertNotEquals(null, node.fields, 'Fields should be initialized');
        System.assertNotEquals(null, node.children, 'Children should be initialized');
        System.assertEquals(0, node.fields.size(), 'Fields should be empty');
        System.assertEquals(0, node.children.size(), 'Children should be empty');
    }

    @IsTest
    static void testHierarchyResultConstructor() {
        Test.startTest();
        AccountHierarchyController.HierarchyResult result = new AccountHierarchyController.HierarchyResult();
        Test.stopTest();

        System.assertNotEquals(null, result.nodes, 'Nodes should be initialized');
        System.assertEquals(0, result.nodes.size(), 'Nodes should be empty');
    }

    @IsTest
    static void testGetAccountHierarchy_MultipleFieldTypes() {
        Account root = [SELECT Id FROM Account WHERE Name = 'Test Root' LIMIT 1];
        root.AnnualRevenue = 1000000;
        root.NumberOfEmployees = 500;
        update root;

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(root.Id, new List<String>{'Name', 'Industry', 'AnnualRevenue', 'NumberOfEmployees'});
        Test.stopTest();

        AccountHierarchyController.HierarchyNode rootNode = result.nodes[0];
        System.assertEquals('Test Root', rootNode.fields.get('Name'), 'Name should be populated');
        System.assertEquals('Technology', rootNode.fields.get('Industry'), 'Industry should be populated');
        System.assertEquals(1000000, rootNode.fields.get('AnnualRevenue'), 'AnnualRevenue should be populated');
        System.assertEquals(500, rootNode.fields.get('NumberOfEmployees'), 'NumberOfEmployees should be populated');
    }

    @IsTest
    static void testGetAccountHierarchy_BulkChildren() {
        Account parent = new Account(Name = 'Bulk Parent');
        insert parent;

        List<Account> children = new List<Account>();
        for (Integer i = 0; i < 50; i++) {
            children.add(new Account(Name = 'Child ' + i, ParentId = parent.Id));
        }
        insert children;

        Test.startTest();
        AccountHierarchyController.HierarchyResult result =
            AccountHierarchyController.getAccountHierarchy(parent.Id, new List<String>{'Name'});
        Test.stopTest();

        System.assertEquals(1, result.nodes.size(), 'Should have 1 root node');
        System.assertEquals(50, result.nodes[0].children.size(), 'Parent should have 50 children');
    }
}
