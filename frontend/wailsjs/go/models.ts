export namespace main {
	
	export class AgentInput {
	    name: string;
	    description: string;
	    prompt: string;
	    model: string;
	
	    static createFrom(source: any = {}) {
	        return new AgentInput(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.description = source["description"];
	        this.prompt = source["prompt"];
	        this.model = source["model"];
	    }
	}
	export class AgentView {
	    id: string;
	    name: string;
	    description: string;
	    model: string;
	    source: string;
	    managed: boolean;
	    enabledOn: Record<string, boolean>;
	
	    static createFrom(source: any = {}) {
	        return new AgentView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.description = source["description"];
	        this.model = source["model"];
	        this.source = source["source"];
	        this.managed = source["managed"];
	        this.enabledOn = source["enabledOn"];
	    }
	}
	export class HostView {
	    id: string;
	    name: string;
	    description: string;
	    installed: boolean;
	    status: string;
	    configPath: string;
	    skillPath: string;
	    agentPath: string;
	    format: string;
	
	    static createFrom(source: any = {}) {
	        return new HostView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.description = source["description"];
	        this.installed = source["installed"];
	        this.status = source["status"];
	        this.configPath = source["configPath"];
	        this.skillPath = source["skillPath"];
	        this.agentPath = source["agentPath"];
	        this.format = source["format"];
	    }
	}
	export class MCPInput {
	    name: string;
	    description: string;
	    type: string;
	    endpoint: string;
	    command: string;
	    args: string[];
	    env: Record<string, string>;
	    headers: Record<string, string>;
	    secret: string;
	
	    static createFrom(source: any = {}) {
	        return new MCPInput(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.description = source["description"];
	        this.type = source["type"];
	        this.endpoint = source["endpoint"];
	        this.command = source["command"];
	        this.args = source["args"];
	        this.env = source["env"];
	        this.headers = source["headers"];
	        this.secret = source["secret"];
	    }
	}
	export class MCPView {
	    id: string;
	    name: string;
	    description: string;
	    type: string;
	    endpoint: string;
	    command: string;
	    hasCredentials: boolean;
	    managed: boolean;
	    enabledOn: Record<string, boolean>;
	
	    static createFrom(source: any = {}) {
	        return new MCPView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.description = source["description"];
	        this.type = source["type"];
	        this.endpoint = source["endpoint"];
	        this.command = source["command"];
	        this.hasCredentials = source["hasCredentials"];
	        this.managed = source["managed"];
	        this.enabledOn = source["enabledOn"];
	    }
	}
	export class ProjectDeploymentView {
	    enabled: boolean;
	    status: string;
	
	    static createFrom(source: any = {}) {
	        return new ProjectDeploymentView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.enabled = source["enabled"];
	        this.status = source["status"];
	    }
	}
	export class ProviderView {
	    id: string;
	    name: string;
	    envApiKey: string;
	    baseUrl: string;
	    models: string[];
	    modelsFetchedAt: string;
	    modelsError: string;
	
	    static createFrom(source: any = {}) {
	        return new ProviderView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.envApiKey = source["envApiKey"];
	        this.baseUrl = source["baseUrl"];
	        this.models = source["models"];
	        this.modelsFetchedAt = source["modelsFetchedAt"];
	        this.modelsError = source["modelsError"];
	    }
	}
	export class ReferenceView {
	    id: string;
	    name: string;
	    source: string;
	    preview: string;
	
	    static createFrom(source: any = {}) {
	        return new ReferenceView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.source = source["source"];
	        this.preview = source["preview"];
	    }
	}
	export class PromptView {
	    id: string;
	    name: string;
	    source: string;
	    preview: string;
	    enabledOn: Record<string, boolean>;
	    modeByHost: Record<string, string>;
	
	    static createFrom(source: any = {}) {
	        return new PromptView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.source = source["source"];
	        this.preview = source["preview"];
	        this.enabledOn = source["enabledOn"];
	        this.modeByHost = source["modeByHost"];
	    }
	}
	export class SkillView {
	    id: string;
	    name: string;
	    description: string;
	    tracked: boolean;
	    repository: string;
	    source: string;
	    managed: boolean;
	    enabledOn: Record<string, boolean>;
	    modeByHost: Record<string, string>;
	
	    static createFrom(source: any = {}) {
	        return new SkillView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.description = source["description"];
	        this.tracked = source["tracked"];
	        this.repository = source["repository"];
	        this.source = source["source"];
	        this.managed = source["managed"];
	        this.enabledOn = source["enabledOn"];
	        this.modeByHost = source["modeByHost"];
	    }
	}
	export class ProjectView {
	    id: string;
	    name: string;
	    path: string;
	    available: boolean;
	    createdAt: string;
	    deployments: Record<string, any>;
	
	    static createFrom(source: any = {}) {
	        return new ProjectView(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.path = source["path"];
	        this.available = source["available"];
	        this.createdAt = source["createdAt"];
	        this.deployments = source["deployments"];
	    }
	}
	export class Snapshot {
	    root: string;
	    hosts: HostView[];
	    projects: ProjectView[];
	    skills: SkillView[];
	    mcps: MCPView[];
	    agents: AgentView[];
	    prompts: PromptView[];
	    references: ReferenceView[];
	    providers: ProviderView[];
	    lastScan: string;
	
	    static createFrom(source: any = {}) {
	        return new Snapshot(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.root = source["root"];
	        this.hosts = this.convertValues(source["hosts"], HostView);
	        this.projects = this.convertValues(source["projects"], ProjectView);
	        this.skills = this.convertValues(source["skills"], SkillView);
	        this.mcps = this.convertValues(source["mcps"], MCPView);
	        this.agents = this.convertValues(source["agents"], AgentView);
	        this.prompts = this.convertValues(source["prompts"], PromptView);
	        this.references = this.convertValues(source["references"], ReferenceView);
	        this.providers = this.convertValues(source["providers"], ProviderView);
	        this.lastScan = source["lastScan"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class ProjectRemovalResult {
	    snapshot: Snapshot;
	    retained: string[];
	
	    static createFrom(source: any = {}) {
	        return new ProjectRemovalResult(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.snapshot = this.convertValues(source["snapshot"], Snapshot);
	        this.retained = source["retained"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	
	
	export class ProviderInput {
	    name: string;
	    apiKey: string;
	    envApiKey: string;
	    baseUrl: string;
	
	    static createFrom(source: any = {}) {
	        return new ProviderInput(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.apiKey = source["apiKey"];
	        this.envApiKey = source["envApiKey"];
	        this.baseUrl = source["baseUrl"];
	    }
	}
	
	
	export class SkillCandidate {
	    name: string;
	    description: string;
	    path: string;
	    installed: boolean;
	
	    static createFrom(source: any = {}) {
	        return new SkillCandidate(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.description = source["description"];
	        this.path = source["path"];
	        this.installed = source["installed"];
	    }
	}
	export class SkillUpdate {
	    id: string;
	    name: string;
	    status: string;
	
	    static createFrom(source: any = {}) {
	        return new SkillUpdate(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.status = source["status"];
	    }
	}
	

}

