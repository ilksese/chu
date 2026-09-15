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
	export class SkillView {
	    id: string;
	    name: string;
	    description: string;
	    repository: string;
	    version: string;
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
	        this.repository = source["repository"];
	        this.version = source["version"];
	        this.source = source["source"];
	        this.managed = source["managed"];
	        this.enabledOn = source["enabledOn"];
	        this.modeByHost = source["modeByHost"];
	    }
	}
	export class Snapshot {
	    root: string;
	    hosts: HostView[];
	    skills: SkillView[];
	    mcps: MCPView[];
	    agents: AgentView[];
	    lastScan: string;
	
	    static createFrom(source: any = {}) {
	        return new Snapshot(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.root = source["root"];
	        this.hosts = this.convertValues(source["hosts"], HostView);
	        this.skills = this.convertValues(source["skills"], SkillView);
	        this.mcps = this.convertValues(source["mcps"], MCPView);
	        this.agents = this.convertValues(source["agents"], AgentView);
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

}

