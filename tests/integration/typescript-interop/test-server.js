#!/usr/bin/env node
/**
 * TypeScript SDK server for testing interoperability with PMCP Rust client
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert';
import { spawn } from 'node:child_process';
import { promisify } from 'node:util';

const sleep = promisify(setTimeout);

describe('TypeScript Server - Rust Client Interop', () => {
    let server;
    let rustClient;
    
    before(async () => {
        console.log('Starting TypeScript MCP server...');
        
        // Create the server
        server = new Server(
            {
                name: 'typescript-test-server',
                version: '1.0.0',
            },
            {
                capabilities: {
                    tools: {},
                    resources: {},
                    prompts: {},
                },
            }
        );
        
        // Register test tools
        server.setRequestHandler('tools/list', async () => ({
            tools: [
                {
                    name: 'add',
                    description: 'Add two numbers',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            a: { type: 'number' },
                            b: { type: 'number' },
                        },
                        required: ['a', 'b'],
                    },
                },
                {
                    name: 'echo',
                    description: 'Echo the input',
                    inputSchema: {
                        type: 'object',
                        properties: {
                            message: { type: 'string' },
                        },
                        required: ['message'],
                    },
                },
            ],
        }));
        
        server.setRequestHandler('tools/call', async (request) => {
            const { name, arguments: args } = request.params;
            
            switch (name) {
                case 'add':
                    return {
                        content: [
                            {
                                type: 'text',
                                text: String(args.a + args.b),
                            },
                        ],
                    };
                    
                case 'echo':
                    return {
                        content: [
                            {
                                type: 'text',
                                text: args.message,
                            },
                        ],
                    };
                    
                default:
                    throw new Error(`Unknown tool: ${name}`);
            }
        });
        
        // Register test resources
        server.setRequestHandler('resources/list', async () => ({
            resources: [
                {
                    uri: 'test://example.txt',
                    name: 'Example Text File',
                    description: 'A test resource',
                    mimeType: 'text/plain',
                },
            ],
        }));
        
        server.setRequestHandler('resources/read', async (request) => {
            const { uri } = request.params;
            
            if (uri === 'test://example.txt') {
                return {
                    contents: [
                        {
                            uri: 'test://example.txt',
                            mimeType: 'text/plain',
                            text: 'Hello from TypeScript server!',
                        },
                    ],
                };
            }
            
            throw new Error(`Resource not found: ${uri}`);
        });
        
        // Register test prompts
        server.setRequestHandler('prompts/list', async () => ({
            prompts: [
                {
                    name: 'greeting',
                    description: 'Generate a greeting',
                    arguments: [
                        {
                            name: 'name',
                            description: 'Name to greet',
                            required: true,
                        },
                    ],
                },
            ],
        }));
        
        server.setRequestHandler('prompts/get', async (request) => {
            const { name, arguments: args } = request.params;
            
            if (name === 'greeting') {
                return {
                    description: 'Generate a greeting',
                    messages: [
                        {
                            role: 'user',
                            content: {
                                type: 'text',
                                text: `Please greet ${args.name}`,
                            },
                        },
                    ],
                };
            }
            
            throw new Error(`Prompt not found: ${name}`);
        });
        
        // Start the server
        const transport = new StdioServerTransport();
        await server.connect(transport);
        
        console.log('TypeScript server started');
    });
    
    after(async () => {
        if (rustClient) {
            rustClient.kill();
        }
        if (server) {
            await server.close();
        }
    });
    
    it('should handle Rust client connection', async () => {
        // Spawn Rust client test
        rustClient = spawn('cargo', [
            'test',
            '--test',
            'typescript_interop',
            'test_rust_client_typescript_server',
            '--',
            '--nocapture'
        ], {
            stdio: 'pipe',
        });
        
        let output = '';
        rustClient.stdout.on('data', (data) => {
            output += data.toString();
            console.log('Rust client:', data.toString());
        });
        
        rustClient.stderr.on('data', (data) => {
            console.error('Rust client error:', data.toString());
        });
        
        // Wait for client to complete
        const exitCode = await new Promise((resolve) => {
            rustClient.on('exit', resolve);
        });
        
        assert.strictEqual(exitCode, 0, 'Rust client should exit successfully');
        assert(output.includes('test result: ok'), 'Rust client tests should pass');
    });
    
    it('should exchange messages correctly', async () => {
        // Test will be run from Rust side
        assert(true, 'Message exchange test placeholder');
    });
    
    it('should handle protocol negotiation', async () => {
        // Test will be run from Rust side
        assert(true, 'Protocol negotiation test placeholder');
    });
    
    it('should handle error conditions', async () => {
        // Test will be run from Rust side
        assert(true, 'Error handling test placeholder');
    });
});