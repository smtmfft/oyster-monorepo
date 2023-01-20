import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";
import * as awsx from "@pulumi/awsx";


let nidx = 192;

let tags = {
    manager: "pulumi",
    project: `oyster`,
}

let regions: aws.Region[] = [
    "us-east-1",
    "us-east-2",
    "us-west-1",
    "us-west-2",
    "ca-central-1",
    "eu-north-1",
    "eu-west-3",
    "eu-west-2",
    "eu-west-1",
    "eu-central-1",
    "eu-south-1",
    "ap-south-1",
    "ap-northeast-1",
    "ap-northeast-2",
    "ap-southeast-1",
    "ap-southeast-2",
    "ap-east-1",
]

let providers: {[key: string]: aws.Provider} = {}
export let publicDNS: aws.route53.Zone;
let privateDNS: aws.route53.Zone;
let amis: {[key: string]: Promise<aws.GetAmiResult>} = {}
let armAmis: {[key: string]: Promise<aws.GetAmiResult>} = {}
let vpcs: {[key: string]: aws.ec2.Vpc} = {}
let subnets: {[key: string]: aws.ec2.Subnet} = {}
let igs: {[key: string]: aws.ec2.InternetGateway} = {}
let rts: {[key: string]: aws.ec2.RouteTable} = {}
let rtas: {[key: string]: aws.ec2.RouteTableAssociation} = {}
let sgs: {[key: string]: {[key: string]: aws.ec2.SecurityGroup}} = {}


regions.forEach((region, ridx) => {
    // providers
    providers[region] = new aws.Provider(region, {
        region: region,
    })

    // amis
    amis[region] = aws.ec2.getAmi({
        mostRecent: true,
        filters: [{
            name: 'name',
            values: ['ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-????????'],
        }],
        owners: ['099720109477'],  // canonical
    }, {
        provider: providers[region],
    })

    armAmis[region] = aws.ec2.getAmi({
        mostRecent: true,
        filters: [{
            name: 'name',
            values: ['ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-arm64-server-????????'],
        }],
        owners: ['099720109477'],  // canonical
    }, {
        provider: providers[region],
    })

    // vpcs
    vpcs[region] = new aws.ec2.Vpc(`${tags.project}-${region}-vpc`, {
        cidrBlock: `10.${nidx}.${ridx}.0/24`,
        enableDnsHostnames: true,
        enableDnsSupport: true,
        tags: tags,
    }, {
        provider: providers[region],
    })

    // subnets
	subnets[`${region}-s0`] = new aws.ec2.Subnet(`${tags.project}-${region}-s0`, {
		cidrBlock: `10.${nidx}.${ridx}.0/24`,
		mapPublicIpOnLaunch: true,
		tags: tags,
		vpcId: vpcs[region].id,
	}, {
		provider: providers[region],
	});

    // internet gateways
    igs[region] = new aws.ec2.InternetGateway(`${tags.project}-${region}-ig`, {
        vpcId: vpcs[region].id,
        tags: tags,
    }, {
        provider: providers[region],
    });

    // route table
    rts[region] = new aws.ec2.RouteTable(`${tags.project}-${region}-rt`, {
        vpcId: vpcs[region].id,
        tags: tags,
    }, {
        provider: providers[region],
    });

    // ig route
    new aws.ec2.Route(`${tags.project}-${region}-ig-route`, {
        routeTableId: rts[region].id,
        destinationCidrBlock: "0.0.0.0/0",
        gatewayId: igs[region].id,
    }, {
        provider: providers[region],
    })

    // route table associations
	rtas[region] = new aws.ec2.RouteTableAssociation(`${tags.project}-${region}-s0-rta`, {
		subnetId: subnets[`${region}-s0`].id,
		routeTableId: rts[region].id,
	}, {
		provider: providers[region],
	});

    // security groups
    sgs[region] = {}
    sgs[region].oyster = new aws.ec2.SecurityGroup(`${tags.project}-${region}-oyster`, {
        vpcId: vpcs[region].id,
        egress: [{
            cidrBlocks: ['0.0.0.0/0'],
            fromPort: 0,
            toPort: 0,
            protocol: "-1",
        }],
		ingress: [{
            cidrBlocks: ['0.0.0.0/0'],
            fromPort: 0,
            toPort: 0,
            protocol: "tcp",
		}],
        tags: tags,
    }, {
        provider: providers[region],
    });
})

