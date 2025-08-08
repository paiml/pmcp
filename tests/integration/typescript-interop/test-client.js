#!/usr/bin/env node
/**
 * TypeScript SDK client for testing interoperability with PMCP Rust server
 */

import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert';
import { spawn } from 'node:child_process';
import { promisify } from 'node:util';

const sleep = promisify(setTimeout);

describe('TypeScript Client - Rust Server Interop', () => {
    let client;
    let rustServer;
    let transport;
    
    before(async () => {
        console.log('Starting Rust MCP server...');
        
        // Spawn Rust server
        rustServer = spawn('cargo', [
            'run',
            '--example',
            '02_server_basic',
        ], {
            stdio: ['pipe', 'pipe', 'inherit'],
        });
        
        // Wait for server to start
        await sleep(2000);
        
        // Create TypeScript client
        transport = new StdioClientTransport({
            command: 'cargo',
            args: ['run', '--example', '02_server_basic'],
        });
        
        client = new Client(
            {
                name: 'typescript-test-client',
                version: '1.0.0',
            },
            {
                capabilities: {},
            }
        );
        
        // Connect to the server
        await client.connect(transport);
        console.log('TypeScript client connected to Rust server');
    });
    
    after(async () => {
        if (client) {
            await client.close();
        }
        if (rustServer) {
            rustServer.kill();
        }
    });
    
    it('should successfully initialize connection', async () => {
        assert(client.getServerVersion(), 'Server version should be available');
        assert(client.getServerCapabilities(), 'Server capabilities should be available');
        
        const serverInfo = client.getServerVersion();
        console.log('Server info:', serverInfo);
        assert.strictEqual(typeof serverInfo.name, 'string');
        assert.strictEqual(typeof serverInfo.version, 'string');
    });
    
    it('should list and call tools', async () => {
        // List tools
        const toolsResult = await client.listTools();
        assert(Array.isArray(toolsResult.tools), 'Tools should be an array');
        console.log(`Found ${toolsResult.tools.length} tools`);
        
        // Find the echo tool
        const echoTool = toolsResult.tools.find(t => t.name === 'echo');
        if (echoTool) {
            // Call the echo tool
            const result = await client.callTool('echo', {
                message: 'Hello from TypeScript!',
            });
            
            assert(result.content, 'Tool should return content');
            assert(result.content.length > 0, 'Content should not be empty');
            
            const textContent = result.content.find(c => c.type === 'text');
            assert(textContent, 'Should have text content');
            assert.strictEqual(
                textContent.text,
                'Hello from TypeScript!',
                'Echo should return the same message'
            );
        }
    });
    
    it('should list and read resources', async () => {
        // List resources
        const resourcesResult = await client.listResources();
        assert(Array.isArray(resourcesResult.resources), 'Resources should be an array');
        console.log(`Found ${resourcesResult.resources.length} resources`);
        
        if (resourcesResult.resources.length > 0) {
            const firstResource = resourcesResult.resources[0];
            
            // Read the resource
            const readResult = await client.readResource(firstResource.uri);
            assert(readResult.contents, 'Should have contents');
            assert(readResult.contents.length > 0, 'Contents should not be empty');
            
            const content = readResult.contents[0];
            console.log('Resource content:', content);
            assert(content.uri, 'Content should have URI');
            assert(content.mimeType || content.text, 'Content should have data');
        }
    });
    
    it('should list and get prompts', async () => {
        // List prompts
        const promptsResult = await client.listPrompts();
        assert(Array.isArray(promptsResult.prompts), 'Prompts should be an array');
        console.log(`Found ${promptsResult.prompts.length} prompts`);
        
        if (promptsResult.prompts.length > 0) {
            const firstPrompt = promptsResult.prompts[0];
            
            // Get the prompt
            const args = {};
            if (firstPrompt.arguments) {
                for (const arg of firstPrompt.arguments) {
                    if (arg.required) {
                        args[arg.name] = 'test-value';
                    }
                }
            }
            
            const promptResult = await client.getPrompt(firstPrompt.name, args);
            assert(promptResult, 'Should get prompt result');
            assert(promptResult.messages, 'Prompt should have messages');
            assert(promptResult.messages.length > 0, 'Messages should not be empty');
            
            console.log('Prompt messages:', promptResult.messages);
        }
    });
    
    it('should handle errors gracefully', async () => {
        try {
            // Try to call a non-existent tool
            await client.callTool('non-existent-tool', {});
            assert.fail('Should have thrown an error');
        } catch (error) {
            assert(error, 'Should catch error');
            console.log('Expected error:', error.message);
        }
        
        try {
            // Try to read a non-existent resource
            await client.readResource('non-existent://resource');
            assert.fail('Should have thrown an error');
        } catch (error) {
            assert(error, 'Should catch error');
            console.log('Expected error:', error.message);
        }
    });
    
    it('should handle concurrent requests', async () => {
        const promises = [];
        
        // Make multiple concurrent requests
        for (let i = 0; i < 5; i++) {
            promises.push(client.listTools());
            promises.push(client.listResources());
            promises.push(client.listPrompts());
        }
        
        const results = await Promise.all(promises);
        assert.strictEqual(results.length, 15, 'All requests should complete');
        
        // Verify all results are valid
        for (let i = 0; i < results.length; i++) {
            assert(results[i], `Result ${i} should not be null`);
        }
    });
    
    it('should respect protocol version', async () => {
        const serverCaps = client.getServerCapabilities();
        console.log('Server capabilities:', serverCaps);
        
        // Check protocol version if available
        const serverInfo = client.getServerVersion();
        if (serverInfo.protocolVersion) {
            assert(
                ['2024-11-05', '2025-03-26', '2025-06-18'].includes(serverInfo.protocolVersion),
                `Protocol version ${serverInfo.protocolVersion} should be supported`
            );
        }
    });
});